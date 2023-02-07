struct CustomMaterial {
    color: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> material: CustomMaterial;
@group(1) @binding(1)
var base_color_texture: texture_2d<f32>;
@group(1) @binding(2)
var base_color_sampler: sampler;

@fragment
fn fragment(
    #import bevy_pbr::mesh_vertex_output
) -> @location(0) vec4<f32> {
    var tiled_uv: vec2<f32>;
    var tiled_uv_x: f32;
    var tiled_uv_y: f32;
    tiled_uv_x = fract(uv.x * 10.0);
    tiled_uv_y = fract(uv.y * 7.0);
    tiled_uv = vec2(tiled_uv_x,tiled_uv_y);
    return textureSample(base_color_texture, base_color_sampler, tiled_uv);
}