use crate::heart::*;
use narui_derive::widget;

use lyon::{
    math::rect,
    path::{builder::*, Winding},
    tessellation::path::{builder::BorderRadii, path::Builder},
};
use std::sync::Arc;
use stretch::{
    geometry::Size,
    style::{Dimension, Style},
};

#[widget(border_radius = 7.5, color = crate::theme::BG_LIGHT, style = Default::default())]
pub fn rounded_rect(
    border_radius: f32,
    color: Color,
    style: Style,
    children: Vec<Widget>,
) -> Widget {
    let path_gen = Arc::new(move |size: Size<f32>| {
        let mut builder = Builder::new();
        builder.add_rounded_rectangle(
            &rect(0.0, 0.0, size.width, size.height),
            &BorderRadii {
                top_left: border_radius,
                top_right: border_radius,
                bottom_left: border_radius,
                bottom_right: border_radius,
            },
            Winding::Positive,
        );
        builder.build()
    });
    Widget::render_object(RenderObject::Path { path_gen, color }, children, style)
}