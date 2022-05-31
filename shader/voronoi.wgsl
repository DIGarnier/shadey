// pain points

struct GuiControlled {
    color_mode: u32;
    number: u32; 
    enhance_factor: f32;
    speed: f32;
    object_color: vec3<f32>;
    point_color: vec3<f32>;
    influence: f32;
    distance_percent: f32;  
};


//Generate 2 somewhat random numbers
fn random2(p: vec2<f32>, i: f32) -> vec2<f32> {
    return fract(sin(vec2<f32>(dot(p,vec2<f32>(15.2,i)),dot(p,vec2<f32>(26.0,18.0))))*4785.3);
}

//Generate pseudo random numbers, following "The book of Shaders"
fn random(st: vec2<f32>) -> f32 {
    return fract(sin(dot(st.xy,vec2<f32>(12.9898,78.233)))*43758.5453123);
}

fn taxicab_distance(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return abs(a.x-b.x) + abs(a.y-b.y);
}

fn cheby_distance(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return max(abs(a.x-b.x),abs(a.y-b.y));
}

fn cosine_distance(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return 1.0 - dot(a,b)/(length(a)*length(b));
}

fn minkowski_distance(a: vec2<f32>, b: vec2<f32>, p: f32) -> f32 {
    return pow(pow(abs(a.x-b.x), p) + pow(abs(a.y-b.y), p), 1.0/p);
}



// Fragment shader
[[stage(fragment)]]
fn fs_main(vo: VertexOutput) -> [[location(0)]] vec4<f32> {
    var pos = screen_coords(vo, vec2<f32>(0.0));
    var color = vec3<f32>(0.0);
    var min_dist = 10.0;

    var points: array<vec2<f32>, 100>;

    var id = 0u;

    for (var idx = 0u; idx < number(); idx=idx+1u) {
        let i = idx + 1u;
        points[idx] = random2(vec2<f32>(f32(i), f32(i)), f32(i)/323132.0) + 
            normalize(random2(vec2<f32>(-f32(i), f32(i)), f32(i)))/1.5*sin(time()*speed());
        let dist = distance(pos, points[idx])*distance_percent() + cosine_distance(pos, points[idx])*(1.0-distance_percent());
        // let dist = minkowski_distance(pos, points[idx], distance_percent()*4.0 - 2.0);
        if (dist < min_dist) {
            min_dist = dist;
            id = i;
        }
    }

    // color = color + min_dist;
    let rando_noise = vec3<f32>(random(vec2<f32>(f32(id))),random(vec2<f32>(f32(id+1u))),random(vec2<f32>(f32(id+2u))));
    color = object_color() + normalize(rando_noise)*influence();

    
    return vec4<f32>(color,1.);
}