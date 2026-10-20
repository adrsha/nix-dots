// Inverse rounded inset: opaque corners and a continuous 4px black frame.
@fragment
fn main(input: Corner) -> @location(0) vec4<f32> {
    let p=input.uv*size;
    let t=values[0].x;
    let r=min(values[0].y,min(size.x,size.y)*0.5-t);
    let q=abs(p-size*0.5)-(size*0.5-vec2<f32>(t+r));
    let d=length(max(q,vec2<f32>(0.0)))+min(max(q.x,q.y),0.0)-r;
    return vec4<f32>(values[1].rgb,smoothstep(-0.6,0.6,d));
}
