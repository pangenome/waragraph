#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in flat uint i_node_id;

layout (location = 0) out vec4 f_color;

layout (location = 1) out uint f_node_id;
layout (location = 2) out vec2 f_uv;

layout (set = 1, binding = 0) readonly buffer NodeData {
  float values[];
} data;

layout (set = 1, binding = 1) uniform sampler u_sampler;
layout (set = 1, binding = 2) uniform texture1D u_colors;

layout (set = 1, binding = 3) uniform ColorMap {
  float min_val;
  float max_val;
  float min_color;
  float max_color;
} u_color_map;

float color_map_position(float v) {
  float range = u_color_map.max_val - u_color_map.min_val;
  float v_n = range > 0.0
    ? clamp((v - u_color_map.min_val) / range, 0.0, 1.0)
    : 0.0;

  return clamp(mix(u_color_map.min_color, u_color_map.max_color, v_n), 0.0, 1.0);
}

void main() {
  float v = data.values[i_node_id];

  vec4 color = texture(sampler1D(u_colors, u_sampler), color_map_position(v));

  f_color = color;

  // increment because the background is all zero & changing that
  // would require some engine changes that i'm too lazy to do rn
  f_node_id = i_node_id + 1;
  f_uv = i_uv;
}
