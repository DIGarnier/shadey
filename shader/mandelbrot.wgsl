// pain points
// - graph widget x-y
// load textures?
struct GuiControlled {
    timespan: u32;
    scale: f32;
    x: f32;
    y: f32;
};

// Generate 2 somewhat random numbers
fn random2(p: vec2<f32>, i: f32) -> vec2<f32> {
    return fract(sin(vec2<f32>(dot(p,vec2<f32>(15.2,i)),dot(p,vec2<f32>(26.0,18.0))))*4785.3);
}

// Generate pseudo random numbers, following "The book of Shaders"
fn random(st: vec2<f32>) -> f32 {
    return fract(sin(dot(st.xy,vec2<f32>(12.9898,78.233)))*43758.5453123);
}

fn sqr_imaginary(complex: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        complex.x * complex.x - complex.y * complex.y,
        2.0 * complex.x * complex.y
    );
}

fn mandelbrot(max_iters: u32, coord: vec2<f32>) -> vec3<f32> {
	var z_n = vec2<f32>(0.0, 0.0);
	for(var i = 0u ; i < max_iters ; i=i+1u) {
		z_n = sqr_imaginary(z_n) + coord;
		if(dot(z_n, z_n) > 64.0) {
            let smooth_it = f32(i) - log2(log2(dot(z_n,z_n))/(log2(64.0)))/log2(2.0);
            return 0.5 + 0.5*cos( 3.0 + smooth_it*0.075*2.0 + vec3<f32>(0.0,0.6,1.0));
		}
	}
	return vec3<f32>(0.0);
}

fn smoothstep(x: f32) -> f32 {
    return x*x*x/(3.0*x*x - (3.0 * x) + 1.0);
}


// Fragment shader
[[stage(fragment)]]
fn fs_main(vo: VertexOutput) -> [[location(0)]] vec4<f32> {
    var mouse_pos = toggle_mouse_pos();
    let total_t = 100.0;
    let t = fract(time()/total_t) * 2.0;
    var real_t = smoothstep(t);
    if (t > 1.0) {
        real_t = smoothstep(2.0 - t);
    }
    
    var anim = clamp(real_t, 0.00006, 1.0);
    var col = vec3<f32>(0.0);
    let AA = 4u;

    for (var m=0u; m<AA; m=m+1u) {
        for (var n=0u; n<AA; n=n+1u) {
            let coords = vec2<f32>(0.5) + 
                (vec2<f32>(f32(m),f32(n)) - 0.5* f32(AA)) / f32(AA*1000u);

            var p = screen_coords(vo, coords); 
            var pos = vec2<f32>(-0.83, 0.2) +  (p * 3.0)*anim;
            
            col = col + mandelbrot(timespan(), pos);
        }
    }
    
    col = col / f32(AA*AA);
    
    return vec4<f32>(col,1.);
}