//! AppKit application delegate bridge for open-file, open-URL, and reopen events.

use incular_platform::{ApplicationActivation, DocumentActivation, UrlActivation};
use objc2::{
    ClassType, DeclaredClass, declare_class, msg_send_id, mutability, rc::Retained,
    runtime::ProtocolObject,
};
use objc2_app_kit::{NSApplication, NSApplicationDelegate};
use objc2_foundation::{MainThreadMarker, NSArray, NSObject, NSObjectProtocol, NSString, NSURL};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};
use url::Url;

struct ActivationIvars {
    deliver: Arc<dyn Fn(ApplicationActivation) + Send + Sync>,
}

declare_class!(
    struct IncularApplicationActivationDelegate;

    // SAFETY: NSObject has no additional subclassing invariants. AppKit owns
    // application-delegate callbacks on the main thread, so MainThreadOnly is
    // the correct mutability marker for this bridge.
    unsafe impl ClassType for IncularApplicationActivationDelegate {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "IncularApplicationActivationDelegate";
    }

    impl DeclaredClass for IncularApplicationActivationDelegate {
        type Ivars = ActivationIvars;
    }

    unsafe impl NSObjectProtocol for IncularApplicationActivationDelegate {}

    unsafe impl NSApplicationDelegate for IncularApplicationActivationDelegate {
        #[method(application:openURLs:)]
        fn application_open_urls(&self, _application: &NSApplication, urls: &NSArray<NSURL>) {
            for activation in normalize_urls(urls) {
                (self.ivars().deliver)(activation);
            }
        }

        #[method(application:openFiles:)]
        fn application_open_files(
            &self,
            _application: &NSApplication,
            filenames: &NSArray<NSString>,
        ) {
            let paths = (0..filenames.count())
                .map(|index| {
                    // SAFETY: the index is generated from this NSArray's exact
                    // count and AppKit keeps the array alive for the callback.
                    let filename = unsafe { filenames.objectAtIndex(index) };
                    PathBuf::from(filename.to_string())
                })
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                (self.ivars().deliver)(ApplicationActivation::OpenFiles(
                    DocumentActivation::from_paths(paths),
                ));
            }
        }

        #[method(applicationShouldHandleReopen:hasVisibleWindows:)]
        fn application_should_handle_reopen(
            &self,
            _application: &NSApplication,
            _has_visible_windows: bool,
        ) -> bool {
            (self.ivars().deliver)(ApplicationActivation::Reopen);
            true
        }
    }
);

impl IncularApplicationActivationDelegate {
    fn new(
        deliver: Arc<dyn Fn(ApplicationActivation) + Send + Sync>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = mtm.alloc();
        let this = this.set_ivars(ActivationIvars { deliver });
        // SAFETY: `this` is a freshly allocated NSObject subclass with all Rust
        // ivars initialized above; NSObject's `init` is its designated init.
        unsafe { msg_send_id![super(this), init] }
    }
}

fn normalize_urls(urls: &NSArray<NSURL>) -> Vec<ApplicationActivation> {
    enum Batch {
        Files(Vec<PathBuf>),
        Urls(Vec<Url>),
    }

    fn flush(batch: Batch, activations: &mut Vec<ApplicationActivation>) {
        match batch {
            Batch::Files(paths) if !paths.is_empty() => activations.push(
                ApplicationActivation::OpenFiles(DocumentActivation::from_paths(paths)),
            ),
            Batch::Urls(urls) if !urls.is_empty() => {
                activations.push(ApplicationActivation::OpenUrls(UrlActivation::new(urls)))
            }
            Batch::Files(_) | Batch::Urls(_) => {}
        }
    }

    let mut activations = Vec::new();
    let mut batch: Option<Batch> = None;
    for index in 0..urls.count() {
        // SAFETY: index is bounded by this NSArray's count and the callback
        // retains the array for the duration of normalization.
        let native = unsafe { urls.objectAtIndex(index) };
        // SAFETY: NSURL accessors return retained Foundation values and do not
        // mutate AppKit state. This callback runs on the AppKit main thread.
        let item = unsafe {
            if native.isFileURL() {
                native
                    .path()
                    .map(|path| EitherActivation::File(PathBuf::from(path.to_string())))
            } else {
                native.absoluteString().and_then(|value| {
                    Url::parse(&value.to_string())
                        .ok()
                        .map(EitherActivation::Url)
                })
            }
        };
        let Some(item) = item else {
            continue;
        };
        match (batch.take(), item) {
            (Some(Batch::Files(mut paths)), EitherActivation::File(path)) => {
                paths.push(path);
                batch = Some(Batch::Files(paths));
            }
            (Some(Batch::Urls(mut parsed)), EitherActivation::Url(url)) => {
                parsed.push(url);
                batch = Some(Batch::Urls(parsed));
            }
            (Some(previous), EitherActivation::File(path)) => {
                flush(previous, &mut activations);
                batch = Some(Batch::Files(vec![path]));
            }
            (Some(previous), EitherActivation::Url(url)) => {
                flush(previous, &mut activations);
                batch = Some(Batch::Urls(vec![url]));
            }
            (None, EitherActivation::File(path)) => batch = Some(Batch::Files(vec![path])),
            (None, EitherActivation::Url(url)) => batch = Some(Batch::Urls(vec![url])),
        }
    }
    if let Some(batch) = batch {
        flush(batch, &mut activations);
    }
    activations
}

enum EitherActivation {
    File(PathBuf),
    Url(Url),
}

#[derive(Clone, Default)]
pub(crate) struct MacosActivationState {
    inner: Rc<ActivationInner>,
}

#[derive(Default)]
struct ActivationInner {
    delegate: RefCell<Option<Retained<IncularApplicationActivationDelegate>>>,
}

impl MacosActivationState {
    pub(crate) fn start_watch(&self, deliver: Arc<dyn Fn(ApplicationActivation) + Send + Sync>) {
        if self.inner.delegate.borrow().is_some() {
            return;
        }
        let mtm = MainThreadMarker::new()
            .expect("AppKit application activation watch must be installed on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        let delegate = IncularApplicationActivationDelegate::new(deliver, mtm);
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        *self.inner.delegate.borrow_mut() = Some(delegate);
    }
}

impl Drop for ActivationInner {
    fn drop(&mut self) {
        if self.delegate.get_mut().is_none() {
            return;
        }
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).setDelegate(None);
        }
        self.delegate.get_mut().take();
    }
}
