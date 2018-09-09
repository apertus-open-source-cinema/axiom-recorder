pub fn debayer(raw_image: Image, context: &Facade) -> Texture2d {
    let target_texture = Texture2d::empty_with_format(
        context,
        UncompressedFloatFormat::U8U8U8U8,
        MipmapsOption::NoMipmap,
        raw_image.width,
        raw_image.height,
    ).unwrap();

    let source_texture = Texture2d::new(
        context,
        texture::RawImage2d {
            data: Cow::from(raw_image.data),
            width: raw_image.width, height: raw_image.height,
            format: texture::ClientFormat::U8
        },
    ).unwrap();

    let program = Program::from_source(
        context,
        gl_util::PASSTHROUGH_VERTEX_SHADER_SRC,
        include_str!("debayer.frag"),
        None,
    ).unwrap();

    target_texture.as_surface().draw(
        &gl_util::Vertex::trinagle_strip_surface(context, 1.0, 1.0),
        &index::NoIndices(index::PrimitiveType::TriangleStrip),
        &program,
        &uniform! {raw_image: &source_texture},
        &Default::default(),
    ).unwrap();

    target_texture
}
