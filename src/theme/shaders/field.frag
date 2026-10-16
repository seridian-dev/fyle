in vec2 uv;
out vec4 out_color;
uniform vec2 resolution;
uniform float time;
uniform float intensity;
uniform vec3 accent;
void main() {
    vec2 pixel=uv*resolution;
    vec2 grid=abs(fract(pixel/48.0)-0.5);
    float lines=smoothstep(0.475,0.5,max(grid.x,grid.y));
    float scan=pow(max(0.0,sin(uv.y*70.0-time*0.18)),30.0);
    vec2 drift=vec2(sin(time*0.07),cos(time*0.06))*0.08;
    float cyan_haze=exp(-5.0*length((uv-vec2(0.15,0.75)-drift)*vec2(1.0,1.3)));
    float pink_haze=exp(-7.0*length(uv-vec2(0.95,0.15)+drift));
    vec3 color=vec3(0.025,0.035,0.055);
    color+=intensity*(accent*(0.045*lines+0.055*cyan_haze+0.008*scan)+vec3(1.0,0.0,0.5)*0.04*pink_haze);
    out_color=vec4(color,1.0);
}
