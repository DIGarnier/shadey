
struct GuiControlled {
    u: f32;
    v: f32;
    w: vec2<f32>;
    x: f32;
    d: f32;
    f: f32;
};

[[group(0), binding(1)]]
var<uniform> uni: GuiControlled;


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


// Fragment shader
[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let mouse_pos = mouse_pos();
    let t = time();
    var shape_pos = vec2<f32>(sin(t/1.57), cos(t))/2.5 + 0.5;
    var p = norm_coords(in.texcoords - shape_pos);

    var d = circle(p, 0.1);
    var d1 = box(p, vec2<f32>(0.3, 0.1));
    d = to_ring(d+d1, 0.05);
    var col = vec3<f32>(1.0) - sign(d)*vec3<f32>(shape_pos,0.7);
    col = col * (1.0 - exp(-3.0*abs(d)));
	col = col * (0.8 + 0.2*cos(150.0*d + (t*15.)));
    col = mix(col, vec3<f32>(1.0), 1.0-smoothStep(0.0,0.01,abs(d)));

    return vec4<f32>(col,1.);
}