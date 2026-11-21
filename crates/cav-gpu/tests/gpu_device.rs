//! GPU tier: the device, on the local adapter.

use cav_gpu::{Gpu, wgpu};

#[test]
fn gpu_a_validation_error_is_an_err_not_a_panic() {
    let gpu = Gpu::new().expect("the GPU tier needs an adapter");
    let buffer = |size: u64| {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test"),
            size,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        })
    };
    // Control: work the GPU accepts comes back as Ok with its value.
    assert_eq!(gpu.checked(|| buffer(1024).size()), Ok(1024));
    // A buffer one byte over the device's limit is refused by validation. Nothing that size is
    // allocated; outside `checked` this would reach wgpu's default handler and panic, as the
    // 1.4 GB tick buffer of the S0 review did.
    let too_big = gpu.device.limits().max_buffer_size + 1;
    let why = gpu.checked(|| buffer(too_big)).map(|_| ()).unwrap_err();
    assert!(why.contains("the GPU refused the work"), "{why}");
    // The device is still usable afterwards.
    assert_eq!(gpu.checked(|| buffer(64).size()), Ok(64));
}
