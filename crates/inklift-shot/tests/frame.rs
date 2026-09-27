use inklift_shot::{Frame, Rect};

/// X11 GetImage on a depth-24 visual returns BGRX: blue, green, red, padding.
fn bgrx(pixels: &[(u8, u8, u8)]) -> Vec<u8> {
    pixels.iter().flat_map(|&(r, g, b)| [b, g, r, 0]).collect()
}

#[test]
fn a_frame_reads_bgrx_bytes_as_rgb() {
    let data = bgrx(&[(255, 0, 0), (0, 255, 0), (0, 0, 255), (10, 20, 30)]);
    let f = Frame::from_bgrx(2, 2, data).unwrap();

    assert_eq!(f.pixel(0, 0), [255, 0, 0], "red must not come back as blue");
    assert_eq!(f.pixel(1, 0), [0, 255, 0]);
    assert_eq!(f.pixel(0, 1), [0, 0, 255]);
    assert_eq!(f.pixel(1, 1), [10, 20, 30]);
}

#[test]
fn a_frame_rejects_a_buffer_that_does_not_match_its_dimensions() {
    assert!(Frame::from_bgrx(4, 4, vec![0u8; 10]).is_err());
    assert!(Frame::from_bgrx(0, 4, vec![]).is_err());
}

/// Some servers pad each row; the row stride is then wider than the image.
#[test]
fn a_frame_honours_row_padding() {
    let width = 2u32;
    let stride = 12usize; // 2 pixels of data plus 4 bytes of padding
    let mut data = vec![0u8; stride * 2];
    // row 0, pixel 0 = red; row 1, pixel 1 = green
    data[0..4].copy_from_slice(&[0, 0, 255, 0]);
    data[stride + 4..stride + 8].copy_from_slice(&[0, 255, 0, 0]);

    let f = Frame::with_stride(width, 2, stride, data).unwrap();

    assert_eq!(f.pixel(0, 0), [255, 0, 0]);
    assert_eq!(f.pixel(1, 1), [0, 255, 0]);
}

#[test]
fn cropping_selects_the_requested_pixels() {
    // A 4x4 frame where each pixel encodes its own coordinates.
    let mut px = Vec::new();
    for y in 0..4u8 {
        for x in 0..4u8 {
            px.push((x, y, 0));
        }
    }
    let f = Frame::from_bgrx(4, 4, bgrx(&px)).unwrap();

    let c = f.crop(&Rect::new(1, 2, 2, 2)).unwrap();

    assert_eq!((c.width(), c.height()), (2, 2));
    assert_eq!(c.pixel(0, 0), [1, 2, 0]);
    assert_eq!(c.pixel(1, 0), [2, 2, 0]);
    assert_eq!(c.pixel(0, 1), [1, 3, 0]);
    assert_eq!(c.pixel(1, 1), [2, 3, 0]);
}

#[test]
fn cropping_outside_the_frame_is_refused_rather_than_silently_trimmed() {
    let f = Frame::from_bgrx(4, 4, bgrx(&[(0, 0, 0); 16])).unwrap();
    assert!(f.crop(&Rect::new(3, 3, 4, 4)).is_err(), "must not quietly return less than asked");
    assert!(f.crop(&Rect::new(-1, 0, 2, 2)).is_err());
}

#[test]
fn cropping_the_whole_frame_returns_an_identical_frame() {
    let px: Vec<(u8, u8, u8)> = (0..16u8).map(|i| (i, 255 - i, i / 2)).collect();
    let f = Frame::from_bgrx(4, 4, bgrx(&px)).unwrap();

    let c = f.crop(&Rect::new(0, 0, 4, 4)).unwrap();

    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(c.pixel(x, y), f.pixel(x, y));
        }
    }
}

/// The handoff to the extractor: three `[0,1]` planes in R, G, B order.
#[test]
fn a_frame_converts_to_extractor_planes() {
    let f = Frame::from_bgrx(2, 1, bgrx(&[(255, 128, 0), (0, 0, 0)])).unwrap();

    let planes = f.to_planes();

    assert_eq!((planes[0].width(), planes[0].height()), (2, 1));
    assert!((planes[0].get(0, 0) - 1.0).abs() < 1e-6, "red channel");
    assert!((planes[1].get(0, 0) - 128.0 / 255.0).abs() < 1e-6, "green channel");
    assert!((planes[2].get(0, 0) - 0.0).abs() < 1e-6, "blue channel");
    assert!(planes[0].get(1, 0).abs() < 1e-6);
}

/// The clipboard and PNG writer both want straight RGBA.
#[test]
fn a_frame_exports_rgba_bytes() {
    let f = Frame::from_bgrx(2, 1, bgrx(&[(1, 2, 3), (4, 5, 6)])).unwrap();

    let rgba = f.to_rgba8();

    assert_eq!(rgba, vec![1, 2, 3, 255, 4, 5, 6, 255]);
}
