struct GuiControlled {
    x: f32;
};



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

    var circle_pos = vec2<f32>(sin(time()/1.57), cos(time()))/2.5 + 0.5;
    var p = screen_coords(in, circle_pos);

    var d = circle(p, 0.1);
    var d1 = box(p, vec2<f32>(0.3, 0.1));
    d = to_ring(d+d1, 0.05);
    var col = vec3<f32>(1.0) - sign(d)*vec3<f32>(circle_pos,0.7);

    return vec4<f32>(col,1.);
}