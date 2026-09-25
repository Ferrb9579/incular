//! Incular's build-time parsing of WGPU's internal validation shaders.
//! Runtime validation and backend compilation are unchanged.

fn decode(bytes: &[u8]) -> naga::Module {
    postcard::from_bytes(bytes).expect("embedded WGPU validation shader")
}

pub(crate) fn draw() -> naga::Module {
    decode(include_bytes!(concat!(env!("OUT_DIR"), "/draw.naga")))
}

pub(crate) fn timestamp() -> naga::Module {
    decode(include_bytes!(concat!(env!("OUT_DIR"), "/timestamp.naga")))
}

pub(crate) fn dispatch(limit: u32) -> naga::Module {
    let mut module = decode(include_bytes!(concat!(env!("OUT_DIR"), "/dispatch.naga")));
    let function = &mut module.entry_points[0].function;
    let handle = *function.named_expressions.iter()
        .find(|(_, name)| name.as_str() == "max_compute_workgroups_per_dimension")
        .expect("named dispatch validation limit").0;
    assert!(matches!(function.expressions[handle], naga::Expression::Literal(naga::Literal::U32(u32::MAX))));
    function.expressions[handle] = naga::Expression::Literal(naga::Literal::U32(limit));
    module
}
