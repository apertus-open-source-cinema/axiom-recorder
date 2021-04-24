use crate::heart::*;
use narui_derive::widget;

use stretch::{
    geometry::{Rect, Size},
    style::{AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, Style},
};

#[widget(style = Default::default())]
pub fn container(style: Style, children: Vec<Widget>) -> Widget {
    Widget::layout_block(style, children)
}

#[widget(justify_content = Default::default(), align_items = Default::default(), style = Default::default(), fill_parent = true)]
pub fn column(
    justify_content: JustifyContent,
    align_items: AlignItems,
    fill_parent: bool,
    style: Style,
    children: Vec<Widget>,
) -> Widget {
    let style = Style {
        flex_direction: FlexDirection::Column,
        flex_wrap: FlexWrap::NoWrap,
        size: Size {
            height: if fill_parent { Dimension::Percent(1.0) } else { Default::default() },
            width: if fill_parent { Dimension::Percent(1.0) } else { Default::default() },
        },
        justify_content,
        align_items,
        ..style
    };
    Widget::layout_block(style, children)
}

#[widget(justify_content = Default::default(), align_items = Default::default(), fill_parent = true, style = Default::default())]
pub fn row(
    justify_content: JustifyContent,
    align_items: AlignItems,
    fill_parent: bool,
    style: Style,
    children: Vec<Widget>,
) -> Widget {
    let style = Style {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::NoWrap,
        size: Size {
            height: if fill_parent { Dimension::Percent(1.0) } else { Default::default() },
            width: if fill_parent { Dimension::Percent(1.0) } else { Default::default() },
        },
        justify_content,
        align_items,
        ..style
    };
    Widget::layout_block(style, children)
}

#[widget(all=Default::default(), top_bottom=Default::default(), left_right=Default::default(), top=Default::default(), bottom=Default::default(), left=Default::default(), right=Default::default(), style = Default::default())]
pub fn padding(
    all: Dimension,
    top_bottom: Dimension,
    left_right: Dimension,
    top: Dimension,
    bottom: Dimension,
    left: Dimension,
    right: Dimension,
    style: Style,
    children: Vec<Widget>,
) -> Widget {
    let (mut t, mut b, mut l, mut r) = (all, all, all, all);
    if top_bottom != Dimension::default() {
        t = top_bottom;
        b = top_bottom;
    }
    if left_right != Dimension::default() {
        l = left_right;
        r = left_right;
    }
    if top != Dimension::default() {
        t = top
    }
    if bottom != Dimension::default() {
        b = bottom
    }
    if left != Dimension::default() {
        l = left
    }
    if right != Dimension::default() {
        r = right
    }

    let style = Style { padding: Rect { start: l, end: r, top: t, bottom: b }, ..style };
    Widget::layout_block(style, children)
}

#[widget(width = Default::default(), height = Default::default(), style = Default::default())]
pub fn min_size(
    width: Dimension,
    height: Dimension,
    style: Style,
    children: Vec<Widget>,
) -> Widget {
    let style = Style { min_size: Size { height, width }, ..style };
    Widget::layout_block(style, children)
}
