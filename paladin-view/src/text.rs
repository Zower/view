use cosmic_text::{CacheKey, FontSystem, SubpixelBin};
use femtovg::{
    Atlas, Canvas, DrawCommand, ErrorKind, GlyphDrawCommands, ImageFlags, ImageId, ImageSource,
    Quad, Renderer,
};
use std::collections::HashMap;

use imgref::{Img, ImgRef};
use rgb::RGBA8;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};

const GLYPH_PADDING: u32 = 1;
const GLYPH_MARGIN: u32 = 1;
const TEXTURE_SIZE: usize = 512;

pub fn init_cache() -> RenderCache {
    // Text stuff
    let mut font_system = FontSystem::new();

    let font = include_bytes!("../../assets/JetBrainsMono-Regular.ttf").to_vec();
    font_system.db_mut().load_font_data(font);

    RenderCache {
        font_system,
        scale_context: Default::default(),
        rendered_glyphs: Default::default(),
        glyph_textures: Default::default(),
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RenderedGlyph {
    texture_index: usize,
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
    atlas_x: u32,
    atlas_y: u32,
    color_glyph: bool,
}

pub struct FontTexture {
    atlas: Atlas,
    image_id: ImageId,
}

pub struct RenderCache {
    scale_context: ScaleContext,
    rendered_glyphs: HashMap<CacheKey, Option<RenderedGlyph>>,
    glyph_textures: Vec<FontTexture>,
    pub font_system: FontSystem,
}

impl RenderCache {
    pub fn fill_buffer_to_draw_commands<T: Renderer>(
        &mut self,
        canvas: &mut Canvas<T>,
        buffer: &cosmic_text::Buffer,
        position: (f32, f32),
    ) -> Result<Vec<(cosmic_text::Color, GlyphDrawCommands)>, ErrorKind> {
        let mut alpha_cmd_map: HashMap<cosmic_text::Color, HashMap<usize, DrawCommand>> =
            HashMap::default();
        let mut color_cmd_map = HashMap::default();

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let mut cache_key = glyph.physical((0., 0.), 1.).cache_key;

                let position_x = position.0 + cache_key.x_bin.as_float();
                let position_y = position.1 + cache_key.y_bin.as_float();

                let (position_x, subpixel_x) = SubpixelBin::new(position_x);
                let (position_y, subpixel_y) = SubpixelBin::new(position_y);

                cache_key.x_bin = subpixel_x;
                cache_key.y_bin = subpixel_y;
                // perform cache lookup for rendered glyph
                let Some(rendered) = self.rendered_glyphs.entry(cache_key).or_insert_with(|| {
                    // ...or insert it

                    // do the actual rasterization
                    let font = self
                        .font_system
                        .get_font(cache_key.font_id)
                        .expect("Somehow shaped a font that doesn't exist");
                    let mut scaler = self
                        .scale_context
                        .builder(font.as_swash())
                        .size(f32::from_bits(cache_key.font_size_bits))
                        .hint(true)
                        .build();

                    let offset =
                        Vector::new(cache_key.x_bin.as_float(), cache_key.y_bin.as_float());

                    let rendered = Render::new(&[
                        Source::ColorOutline(0),
                        Source::ColorBitmap(StrikeWith::BestFit),
                        Source::Outline,
                    ])
                    // TODO
                    .format(if true {
                        Format::Subpixel
                    } else {
                        Format::Alpha
                    })
                    .offset(offset)
                    .render(&mut scaler, cache_key.glyph_id);

                    // upload it to the GPU
                    rendered.map(|rendered| {
                        // pick an atlas texture for our glyph
                        let content_w = rendered.placement.width as usize;
                        let content_h = rendered.placement.height as usize;
                        let alloc_w = rendered.placement.width + (GLYPH_MARGIN + GLYPH_PADDING) * 2;
                        let alloc_h =
                            rendered.placement.height + (GLYPH_MARGIN + GLYPH_PADDING) * 2;
                        let used_w = rendered.placement.width + GLYPH_PADDING * 2;
                        let used_h = rendered.placement.height + GLYPH_PADDING * 2;
                        let mut found = None;
                        for (texture_index, glyph_atlas) in
                            self.glyph_textures.iter_mut().enumerate()
                        {
                            if let Some((x, y)) = glyph_atlas
                                .atlas
                                .add_rect(alloc_w as usize, alloc_h as usize)
                            {
                                found = Some((texture_index, x, y));
                                break;
                            }
                        }
                        let (texture_index, atlas_alloc_x, atlas_alloc_y) =
                            found.unwrap_or_else(|| {
                                // if no atlas could fit the texture, make a new atlas tyvm
                                // TODO error handling
                                let mut atlas = Atlas::new(TEXTURE_SIZE, TEXTURE_SIZE);
                                let image_id = canvas
                                    .create_image(
                                        Img::new(
                                            vec![
                                                RGBA8::new(0, 0, 0, 0);
                                                TEXTURE_SIZE * TEXTURE_SIZE
                                            ],
                                            TEXTURE_SIZE,
                                            TEXTURE_SIZE,
                                        )
                                        .as_ref(),
                                        ImageFlags::empty(),
                                    )
                                    .unwrap();
                                let texture_index = self.glyph_textures.len();
                                let (x, y) =
                                    atlas.add_rect(alloc_w as usize, alloc_h as usize).unwrap();
                                self.glyph_textures.push(FontTexture { atlas, image_id });
                                (texture_index, x, y)
                            });

                        let atlas_used_x = atlas_alloc_x as u32 + GLYPH_MARGIN;
                        let atlas_used_y = atlas_alloc_y as u32 + GLYPH_MARGIN;
                        let atlas_content_x = atlas_alloc_x as u32 + GLYPH_MARGIN + GLYPH_PADDING;
                        let atlas_content_y = atlas_alloc_y as u32 + GLYPH_MARGIN + GLYPH_PADDING;

                        let mut src_buf = Vec::with_capacity(content_w * content_h);
                        match rendered.content {
                            Content::Mask => {
                                for chunk in rendered.data.chunks_exact(1) {
                                    src_buf.push(RGBA8::new(chunk[0], 0, 0, 0));
                                }
                            }
                            Content::Color | Content::SubpixelMask => {
                                for chunk in rendered.data.chunks_exact(4) {
                                    src_buf
                                        .push(RGBA8::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                                }
                            }
                        }
                        canvas
                            .update_image::<ImageSource>(
                                self.glyph_textures[texture_index].image_id,
                                ImgRef::new(&src_buf, content_w, content_h).into(),
                                atlas_content_x as usize,
                                atlas_content_y as usize,
                            )
                            .unwrap();

                        RenderedGlyph {
                            texture_index,
                            width: used_w,
                            height: used_h,
                            offset_x: rendered.placement.left,
                            offset_y: rendered.placement.top,
                            atlas_x: atlas_used_x,
                            atlas_y: atlas_used_y,
                            color_glyph: matches!(rendered.content, Content::Color),
                        }
                    })
                }) else {
                    continue;
                };

                let cmd_map = if rendered.color_glyph {
                    &mut color_cmd_map
                } else {
                    alpha_cmd_map
                        .entry(
                            glyph
                                .color_opt
                                .unwrap_or(cosmic_text::Color::rgb(255, 255, 255)),
                        )
                        .or_insert_with(HashMap::default)
                };

                let cmd = cmd_map
                    .entry(rendered.texture_index)
                    .or_insert_with(|| DrawCommand {
                        image_id: self.glyph_textures[rendered.texture_index].image_id,
                        quads: Vec::new(),
                    });

                let mut q = Quad::default();
                let it = 1.0 / TEXTURE_SIZE as f32;

                q.x0 =
                    (position_x + glyph.x as i32 + rendered.offset_x - GLYPH_PADDING as i32) as f32;
                q.y0 = (position_y + run.line_y as i32 + glyph.y as i32
                    - rendered.offset_y
                    - GLYPH_PADDING as i32) as f32;
                q.x1 = q.x0 + rendered.width as f32;
                q.y1 = q.y0 + rendered.height as f32;

                q.s0 = rendered.atlas_x as f32 * it;
                q.t0 = rendered.atlas_y as f32 * it;
                q.s1 = (rendered.atlas_x + rendered.width) as f32 * it;
                q.t1 = (rendered.atlas_y + rendered.height) as f32 * it;

                cmd.quads.push(q);
            }
        }

        if !alpha_cmd_map.is_empty() {
            Ok(alpha_cmd_map
                .into_iter()
                .map(|(color, map)| {
                    (
                        color,
                        GlyphDrawCommands {
                            alpha_glyphs: map.into_values().collect(),
                            color_glyphs: color_cmd_map.drain().map(|(_, cmd)| cmd).collect(),
                        },
                    )
                })
                .collect())
        } else {
            Ok(vec![(
                cosmic_text::Color(0),
                GlyphDrawCommands {
                    alpha_glyphs: vec![],
                    color_glyphs: color_cmd_map.drain().map(|(_, cmd)| cmd).collect(),
                },
            )])
        }
    }
}
