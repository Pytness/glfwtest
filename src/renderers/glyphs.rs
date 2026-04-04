use freetype::{Library, face::LoadFlag};
use glow::HasContext;
use rustybuzz::{Face as RbFace, UnicodeBuffer};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::macros::macs::include_shader;

struct GlyphId(u32);

pub struct ShapedGlyph {
    glyph_id: GlyphId,
    x_advance: f32,
    y_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

struct CachedGlyph {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    width: f32,
    height: f32,
    bearing_x: f32,
    bearing_y: f32,
}

struct GlyphCache {
    cache: HashMap<GlyphId, CachedGlyph>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

pub struct GlyphTexture {
    tex: glow::NativeTexture,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    advance_x: i32, // 26.6 -> already shifted down to pixels
}

pub struct TextRenderer {
    gl: Rc<glow::Context>,
    ft_lib: Library,
    ft_face: freetype::Face,
    rb_face: RbFace<'static>,

    glyphs: RefCell<HashMap<u16, GlyphTexture>>,

    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,

    u_proj: Option<glow::NativeUniformLocation>,
    u_color: Option<glow::NativeUniformLocation>,
    u_tex: Option<glow::NativeUniformLocation>,
}

impl TextRenderer {
    pub fn units_per_em(&self) -> f32 {
        self.rb_face.units_per_em() as f32
    }
    pub unsafe fn new(gl: Rc<glow::Context>, font_bytes: &[u8], px_size: u32) -> Self {
        unsafe {
            // rustybuzz face from raw bytes
            // leak for simplicity in this minimal example
            let bytes = font_bytes.to_vec();
            let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            let rb_face = RbFace::from_slice(leaked, 0).expect("failed to create rustybuzz face");

            // freetype face
            let ft_lib = Library::init().expect("failed to init freetype");

            let ft_face = ft_lib
                .new_memory_face(font_bytes.to_vec(), 0)
                .expect("failed to load freetype face");

            ft_face
                .set_pixel_sizes(0, px_size)
                .expect("failed to set pixel size");

            let program = include_shader!(gl, "font");

            let vao = gl.create_vertex_array().unwrap();
            let vbo = gl.create_buffer().unwrap();

            let u_proj = gl.get_uniform_location(program, "u_proj");
            let u_color = gl.get_uniform_location(program, "u_text_color");
            let u_tex = gl.get_uniform_location(program, "u_tex");

            Self {
                gl,
                ft_lib,
                ft_face,
                rb_face,
                glyphs: RefCell::new(HashMap::new()),
                program,
                vao,
                vbo,
                u_proj,
                u_color,
                u_tex,
            }
        }
    }

    pub fn shape_text(&self, text: &str) -> Vec<ShapedGlyph> {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);

        let shaped = rustybuzz::shape(&self.rb_face, &[], buffer);

        let infos = shaped.glyph_infos();
        let positions = shaped.glyph_positions();

        infos
            .iter()
            .zip(positions.iter())
            .map(|(info, pos)| ShapedGlyph {
                glyph_id: GlyphId(info.glyph_id),
                // rustybuzz positions are in font units; convert to pixels later
                x_advance: pos.x_advance as f32,
                y_advance: pos.y_advance as f32,
                x_offset: pos.x_offset as f32,
                y_offset: pos.y_offset as f32,
            })
            .collect()
    }

    pub unsafe fn get_or_create_glyph(&self, glyph_id: u16) -> Option<&GlyphTexture> {
        if !self.glyphs.borrow().contains_key(&glyph_id) {
            self.ft_face
                .load_glyph(glyph_id as u32, LoadFlag::RENDER)
                .expect("freetype load_glyph failed");

            let slot = self.ft_face.glyph();
            let bitmap = slot.bitmap();

            let width = bitmap.width();
            let height = bitmap.rows();
            let left = slot.bitmap_left();
            let top = slot.bitmap_top();
            let advance_x = (slot.advance().x >> 6) as i32;

            let gl = self.gl.as_ref();

            unsafe {
                let tex = gl.create_texture().unwrap();
                gl.bind_texture(glow::TEXTURE_2D, Some(tex));

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

                self.glyphs.borrow_mut().insert(
                    glyph_id,
                    GlyphTexture {
                        tex,
                        width,
                        height,
                        left,
                        top,
                        advance_x,
                    },
                );
            }
        }

        let glyph = unsafe {
            self.glyphs
                .try_borrow_unguarded()
                .ok()
                .unwrap()
                .get(&glyph_id)
                .unwrap()
        };

        Some(glyph)
    }

    pub unsafe fn draw_text(
        &self,
        text: &str,
        mut pen_x: f32,
        baseline_y: f32,
        px_size: f32,
        color: [f32; 3],
        proj: &[f32; 16],
    ) {
        let shaped = self.shape_text(text);
        let units_per_em = self.units_per_em();

        let gl = self.gl.as_ref();

        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));

            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

            gl.uniform_matrix_4_f32_slice(self.u_proj.as_ref(), false, proj);
            gl.uniform_3_f32(self.u_color.as_ref(), color[0], color[1], color[2]);
            gl.uniform_1_i32(self.u_tex.as_ref(), 0);

            for g in shaped {
                let glyph = self
                    .get_or_create_glyph(g.glyph_id.0 as u16)
                    .expect("failed to get or create glyph");

                let x_offset = hb_to_px(g.x_offset, px_size, units_per_em);
                let y_offset = hb_to_px(g.y_offset, px_size, units_per_em);

                let x = pen_x + x_offset + glyph.left as f32;
                let y = baseline_y - y_offset - glyph.top as f32;

                let w = glyph.width as f32;
                let h = glyph.height as f32;

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

                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(glyph.tex));

                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                    gl.buffer_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        bytemuck::cast_slice(&vertices),
                        glow::DYNAMIC_DRAW,
                    );

                    gl.draw_arrays(glow::TRIANGLES, 0, 6);
                }

                pen_x += hb_to_px(g.x_advance, px_size, units_per_em);
            }

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.use_program(None);
        }
    }
}

impl Drop for TextRenderer {
    fn drop(&mut self) {
        unsafe {
            let gl = self.gl.as_ref();

            for glyph in self.glyphs.borrow().values() {
                gl.delete_texture(glyph.tex);
            }

            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_program(self.program);
        }
    }
}

fn hb_to_px(v: f32, px_size: f32, units_per_em: f32) -> f32 {
    v * (px_size / units_per_em)
}
