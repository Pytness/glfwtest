use std::collections::HashMap;
use std::mem::{offset_of, size_of};
use std::rc::Rc;
use std::sync::LazyLock;

use freetype::{Library, face::LoadFlag};
use glow::HasContext;
use rustybuzz::{Face as RbFace, ShapePlan, UnicodeBuffer};

use crate::font_registry::FontRegistry;
use crate::macros::macs::include_shader;

static FT_LIB: LazyLock<Library> =
    LazyLock::new(|| Library::init().expect("failed to initialize FreeType library"));

pub struct ShapedGlyph {
    pub glyph_id: u32,
    // x_advance/y_advance are provided for completeness (callers doing free-flow layout
    // may use them); the terminal renderer ignores them in favour of a fixed cell width.
    #[allow(dead_code)]
    pub x_advance: f32,
    #[allow(dead_code)]
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

struct GlyphTexture {
    tex: glow::NativeTexture,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
}

pub struct TextRenderer<'a> {
    gl: Rc<glow::Context>,
    // font_data must outlive rb_face; both are dropped explicitly in Drop (rb_face first)
    font_registry: &'a FontRegistry,
    ft_face: freetype::Face,
    rb_face: RbFace<'a>,

    glyphs: HashMap<u32, GlyphTexture>,

    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,

    u_proj: Option<glow::NativeUniformLocation>,
    u_color: Option<glow::NativeUniformLocation>,
    u_tex: Option<glow::NativeUniformLocation>,
    font_size_px: (f32, f32),
    px_size: f32,
    shape_plan: ShapePlan,
    shape_buffer: Option<UnicodeBuffer>,
}

impl<'a> TextRenderer<'a> {
    pub fn units_per_em(&self) -> f32 {
        self.rb_face.units_per_em() as f32
    }

    /// Returns (width, height) of the font at the current pixel size.
    /// This is not the same as the maximum glyph size, but can be used for layout purposes.
    pub fn font_size(&self) -> (f32, f32) {
        self.font_size_px
    }

    pub unsafe fn new(
        gl: Rc<glow::Context>,
        font_registry: &'a FontRegistry,
        px_size: u32,
    ) -> Self {
        // Box the font bytes so they can be freed when the renderer is dropped.
        let font_data: &[u8] = font_registry
            .get_fonts()
            .first()
            .expect("no fonts registered")
            .bytes
            .as_slice();

        // SAFETY: we extend the lifetime to 'static here, but we guarantee that
        // rb_face (which borrows this data) is dropped before font_data in our Drop impl.
        let rb_face = RbFace::from_slice(font_data, 0).expect("failed to create rustybuzz face");

        let ft_face = FT_LIB
            .new_memory_face(font_data.to_vec(), 0)
            .expect("failed to load freetype face");
        ft_face
            .set_pixel_sizes(0, px_size)
            .expect("failed to set pixel size");

        let shape_plan =
            ShapePlan::new(&rb_face, rustybuzz::Direction::LeftToRight, None, None, &[]);

        let metrics = ft_face.size_metrics().expect("failed to get size metrics");
        let cell_width = metrics.max_advance as f32 / 64.0;
        let cell_height = metrics.height as f32 / 64.0;
        let font_size_px = (cell_width, cell_height);

        // Explicit unsafe block required by Rust 2024: unsafe fn bodies no longer
        // implicitly permit unsafe calls without an unsafe{} block.
        let program = unsafe { include_shader!(gl, "font") };

        let vao = unsafe { gl.create_vertex_array().unwrap() };
        let vbo = unsafe { gl.create_buffer().unwrap() };

        let u_proj = unsafe { gl.get_uniform_location(program, "u_proj") };
        let u_color = unsafe { gl.get_uniform_location(program, "u_text_color") };
        let u_tex = unsafe { gl.get_uniform_location(program, "u_font") };

        Self {
            gl,
            font_registry,
            ft_face,
            rb_face,
            glyphs: HashMap::new(),
            program,
            vao,
            vbo,
            u_proj,
            u_color,
            u_tex,
            font_size_px,
            px_size: px_size as f32,
            shape_plan,
            shape_buffer: Some(UnicodeBuffer::new()),
        }
    }

    pub fn shape_text(&mut self, text: &str) -> Vec<ShapedGlyph> {
        let mut buffer = self
            .shape_buffer
            .take()
            .expect("shape_buffer already in use");

        buffer.push_str(text);

        let shaped = rustybuzz::shape_with_plan(&self.rb_face, &self.shape_plan, buffer);

        let infos = shaped.glyph_infos();
        let positions = shaped.glyph_positions();
        let glyphs = infos
            .iter()
            .zip(positions.iter())
            .map(|(info, pos)| ShapedGlyph {
                glyph_id: info.glyph_id,
                // rustybuzz positions are in font units; converted to pixels in draw_text
                x_advance: pos.x_advance as f32,
                y_advance: pos.y_advance as f32,
                x_offset: pos.x_offset as f32,
                y_offset: pos.y_offset as f32,
            })
            .collect();

        self.shape_buffer = Some(shaped.clear());

        glyphs
    }

    /// Ensures the glyph is loaded and cached.
    /// Returns a tuple of (left, top, width, height, tex) to avoid holding a
    /// reference into `self.glyphs` across subsequent `self` accesses.
    fn ensure_glyph(&mut self, glyph_id: u32) -> Option<(i32, i32, i32, i32, glow::NativeTexture)> {
        if !self.glyphs.contains_key(&glyph_id) {
            // SAFETY: requires an active GL context.
            let texture =
                unsafe { Self::load_glyph_texture(self.gl.as_ref(), &self.ft_face, glyph_id)? };
            self.glyphs.insert(glyph_id, texture);
        }
        let g = self.glyphs.get(&glyph_id)?;
        Some((g.left, g.top, g.width, g.height, g.tex))
    }

    /// Rasterises a single glyph with FreeType and uploads it to a GL texture.
    ///
    /// The texture uses the `GL_RED` internal format (one byte per texel) to
    /// minimise GPU memory usage.  The fragment shader samples only the red
    /// channel.
    unsafe fn load_glyph_texture(
        gl: &glow::Context,
        ft_face: &freetype::Face,
        glyph_id: u32,
    ) -> Option<GlyphTexture> {
        ft_face
            .load_glyph(
                glyph_id,
                LoadFlag::RENDER | LoadFlag::TARGET_NORMAL | LoadFlag::FORCE_AUTOHINT,
            )
            .expect("freetype load_glyph failed");

        let slot = ft_face.glyph();
        let bitmap = slot.bitmap();

        let width = bitmap.width();
        let height = bitmap.rows();
        let left = slot.bitmap_left();
        let top = slot.bitmap_top();

        // Explicit unsafe block required by Rust 2024: unsafe fn bodies no longer
        // implicitly permit unsafe calls without an unsafe{} block.
        unsafe {
            let tex = gl.create_texture().ok()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));

            // Grayscale bitmap: one byte per pixel, no alignment padding needed.
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                width,
                height,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(bitmap.buffer())),
            );

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            Some(GlyphTexture {
                tex,
                width,
                height,
                left,
                top,
            })
        }
    }

    pub unsafe fn draw_text(
        &mut self,
        text: &str,
        mut pen_x: f32,
        baseline_y: f32,
        color: [f32; 3],
        proj: &[f32; 16],
    ) -> usize {
        let shaped = self.shape_text(text);
        let units_per_em = self.units_per_em();
        let px_size = self.px_size;

        let stride = size_of::<Vertex>() as i32;
        let uv_offset = offset_of!(Vertex, uv) as i32;

        unsafe {
            let gl = self.gl.as_ref();

            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, uv_offset);

            gl.use_program(Some(self.program));

            gl.enable(glow::BLEND);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );

            gl.uniform_matrix_4_f32_slice(self.u_proj.as_ref(), false, proj);
            gl.uniform_3_f32(self.u_color.as_ref(), color[0], color[1], color[2]);
            gl.uniform_1_i32(self.u_tex.as_ref(), 0);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            for g in &shaped {
                // Extract all glyph data as owned/Copy values so the borrow on
                // `self.glyphs` ends before we re-access other fields of `self`.
                let Some((left, top, width, height, tex)) = self.ensure_glyph(g.glyph_id) else {
                    println!("Warning: glyph ID {} not found in font", g.glyph_id);
                    continue;
                };

                let x_offset = hb_to_px(g.x_offset, px_size, units_per_em);
                let y_offset = hb_to_px(g.y_offset, px_size, units_per_em);

                let x = pen_x + x_offset + left as f32;
                let y = baseline_y - y_offset - top as f32;

                let w = width as f32;
                let h = height as f32;

                if w > 0.0 && h > 0.0 {
                    let vertices = [
                        Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y],
                            uv: [1.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y + h],
                            uv: [1.0, 1.0],
                        },
                        Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y + h],
                            uv: [1.0, 1.0],
                        },
                        Vertex {
                            pos: [x, y + h],
                            uv: [0.0, 1.0],
                        },
                    ];

                    let gl = self.gl.as_ref();
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(tex));

                    gl.buffer_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        bytemuck::cast_slice(&vertices),
                        glow::DYNAMIC_DRAW,
                    );

                    gl.draw_arrays(glow::TRIANGLES, 0, 6);
                }

                // NOTE: due to the way text is rendered in a terminal (in a fixed grid),
                // we ignore the actual x_advance and just move the pen by the cell width.
                pen_x += self.font_size_px.0;
            }

            let gl = self.gl.as_ref();
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.use_program(None);

            shaped.len()
        }
    }
}

impl<'a> Drop for TextRenderer<'a> {
    fn drop(&mut self) {
        unsafe {
            let gl = self.gl.as_ref();

            for glyph in self.glyphs.values() {
                gl.delete_texture(glyph.tex);
            }

            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_program(self.program);

            // Drop rb_face before releasing the font data it references.
            // ManuallyDrop::drop(&mut self.rb_face);
            // ManuallyDrop::drop(&mut self._font_data);
        }
    }
}

fn hb_to_px(v: f32, px_size: f32, units_per_em: f32) -> f32 {
    v * (px_size / units_per_em)
}
