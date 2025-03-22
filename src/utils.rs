//! Isolated set of helper functions.

use web_sys::{
    window as web_window, CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlImageElement,
};

use crate::Vec2;
use web_sys::wasm_bindgen::JsCast;

macro_rules! vec2 {
    ($x_var:expr, $y_var:expr) => {
        Vec2 {
            x: $x_var,
            y: $y_var,
        }
    };
}

/// Convert a point from screen to world space.
pub fn screen_to_world_space(pos: Vec2, window: Vec2) -> Vec2 {
    Vec2 {
        x: -1. + ((pos.x / window.x as f32) * 2.),
        y: -(-1. + ((pos.y / window.y as f32) * 2.)),
    }
}

/// Load an iamge by reading an `img` tag with id `last-image`.
// Most code was generated by ChatGPT (sources unknown)
pub fn load_image_wasm() -> (Vec<u8>, Vec2) {
    let mut result: Vec<u8> = vec![];
    let mut dimensions = vec2!{0., 0.};

    let document: Document = web_window().unwrap().document().unwrap();
    if let Some(img_element) = document.get_element_by_id("last-image") {
        let img = img_element.dyn_into::<HtmlImageElement>().unwrap();

        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        canvas.set_width(img.width());
        canvas.set_height(img.height());

        // get 2D rendering context
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        // draw image onto canvas
        context
            .draw_image_with_html_image_element(&img, 0.0, 0.0)
            .unwrap();

        // extract image data (RGBA pixels)
        let image_data = context
            .get_image_data(0.0, 0.0, img.width() as f64, img.height() as f64)
            .unwrap();
        let pixels = image_data.data();

        // convert js_sys::Uint8ClampedArray to Vec<u8>
        pixels.to_vec();

        dimensions = vec2!(img.width() as f32, img.height() as f32);
        result = pixels.to_vec();
    }

    return (result, dimensions);
}
