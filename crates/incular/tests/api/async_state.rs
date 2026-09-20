use incular::runtime::AsyncState;

#[test]
fn async_state_is_generic_and_maps_success_values() {
    let loading: AsyncState<u32, &'static str> = AsyncState::Loading;
    assert!(loading.is_loading());
    assert!(!loading.is_terminal());

    let ready = AsyncState::<u32, &'static str>::Ready(4_u32).map(|value| value.to_string());
    assert_eq!(ready, AsyncState::Ready("4".to_owned()));

    let error: AsyncState<u32, &'static str> = AsyncState::Error("offline");
    assert!(error.is_error());
    assert!(error.is_terminal());
}
