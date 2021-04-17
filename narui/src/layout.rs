/* The layout pass takes the tree where the leaf nodes are Primitive Widgets and converts it to a list
of primitive PositionedPrimitiveWidget s that can then be handled to the rendering Backend.
 */

use crate::api::{RenderObject, TreeChildren, TreeChildren::Leaf, Widget};
use stretch::{
    geometry::{Point, Rect, Size},
    node::Node,
    number::Number,
    style::*,
    Error,
    Stretch,
};


#[derive(Debug)]
pub struct PositionedRenderObject {
    render_object: RenderObject,
    position: Point<f32>,
    size: Size<f32>,
    z_index: i32,
}

pub fn do_layout(top: Widget) -> Result<Vec<PositionedRenderObject>, stretch::Error> {
    let mut stretch = stretch::node::Stretch::new();
    let mut stretch_render_object_list = vec![];
    let top_node = add_widget_to_stretch(top, &mut stretch, &mut stretch_render_object_list)?;

    stretch.compute_layout(
        top_node,
        Size { width: Number::Defined(100.0), height: Number::Defined(100.0) },
    )?;
    println!("{}", print_layout(&stretch, top_node));


    Ok(stretch_render_object_list
        .into_iter()
        .map(|(node, render_object)| {
            let layout = stretch.layout(node)?;
            Ok(PositionedRenderObject {
                render_object,
                position: layout.location,
                size: layout.size,
                z_index: 0,
            })
        })
        .collect::<Result<_, _>>()?)
}
fn add_widget_to_stretch(
    widget: Widget,
    stretch: &mut Stretch,
    stretch_render_object_list: &mut Vec<(Node, RenderObject)>,
) -> Result<Node, Error> {
    let (children, render_object) = match widget.children {
        TreeChildren::Children(children) => (children, None),
        TreeChildren::Leaf(render_object) => (vec![], Some(render_object)),
    };
    let mut node_children = Vec::with_capacity(children.len());
    for child in children {
        node_children.push(add_widget_to_stretch(child, stretch, stretch_render_object_list)?);
    }
    let node = stretch.new_node(widget.style, node_children)?;
    if let Some(render_object) = render_object {
        stretch_render_object_list.push((node, render_object))
    }

    Ok(node)
}


fn indent(text: String, indent_str: String) -> String {
    text.lines().into_iter().map(|line| format!("{}{}\n", indent_str, line)).collect()
}

pub fn print_layout(stretch: &Stretch, top_node: Node) -> String {
    let mut to_return = format!("{:?}\n", stretch.layout(top_node));
    for child in stretch.children(top_node).unwrap() {
        to_return += indent(print_layout(&stretch, child), "    ".to_owned()).as_str();
    }
    to_return
}
