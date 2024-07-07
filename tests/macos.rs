#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn it_adds_two() {
    assert_eq!(4, add_two(2));
}