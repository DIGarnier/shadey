
[[block]]
struct InfoUniform {
    window_size: vec2<u32>;
    mouse: vec2<u32>;
    time: f32;
};
[[group(0), binding(0)]]
var<uniform> general_info: InfoUniform;


// Vertex shader

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] texcoords: vec2<f32>;
};


[[stage(vertex)]]
fn vs_main(
    [[builtin(vertex_index)]] in_vertex_index: u32,
) -> VertexOutput {
    var vertices = array<vec2<f32>,3>(vec2<f32>(-1.,-1.), vec2<f32>(3.,-1.), vec2<f32>(-1., 3.));
    var out: VertexOutput;
    out.clip_position = vec4<f32>(vertices[in_vertex_index], 0.0, 1.0);
    out.texcoords = 0.5 * out.clip_position.xy + vec2<f32>(0.5);
    return out;
}

fn to_ring(d:f32, r:f32 ) -> f32
{
  return abs(d) - r;
}

fn circle(p: vec2<f32>, r: f32) -> f32
{
    return length(p) - r;
}

fn box(p: vec2<f32>, b: vec2<f32> ) -> f32
{
    let d = abs(p)-b;
    return length(max(d,vec2<f32>(0.0))) + min(max(d.x,d.y),0.0);
}

fn norm_coords(texcoords: vec2<f32>, resolution: vec2<u32>) -> vec2<f32> {
    let ratio = f32(resolution.x)/f32(resolution.y);
    return vec2<f32>(texcoords.x*ratio, texcoords.y);
}

fn get_mouse_pos() -> vec2<f32> {
    var mouse_pos = vec2<f32>(general_info.mouse.xy)/vec2<f32>(general_info.window_size.xy);
    mouse_pos.y = 1. - mouse_pos.y;
    return mouse_pos;
}

// Fragment shader
[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    var mouse_pos = get_mouse_pos();
    var circle_pos = vec2<f32>(sin(general_info.time/1.57), cos(general_info.time))/2.5 + 0.5;
    var p = norm_coords(in.texcoords - circle_pos, general_info.window_size);

    var d = circle(p, 0.1);
    var d1 = box(p, vec2<f32>(0.3, 0.1));
    d = to_ring(d+d1, 0.05);
    var col = vec3<f32>(1.0) - sign(d)*vec3<f32>(circle_pos,0.7);
    col = col * (1.0 - exp(-3.0*abs(d)));
	col = col * (0.8 + 0.2*cos(150.0*d + (general_info.time*15.)));
    col = mix(col, vec3<f32>(1.0), 1.0-smoothStep(0.0,0.01,abs(d)));

    return vec4<f32>(col,1.);
}