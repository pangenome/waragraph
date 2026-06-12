#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 1) in flat uint i_slot_id;

layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) readonly buffer DataBuf {
  uint total_size;
  uint row_size;
  float values[];
} u_data;

layout (set = 1, binding = 1) uniform sampler u_sampler;
layout (set = 1, binding = 2) uniform texture1D u_colors;

layout (set = 1, binding = 3) uniform ColorMap {
  float min_val;
  float max_val;
  float min_color;
  float max_color;
} u_color_map;

struct SlotUniform {
  vec2 ab;
  uint bin_count;
};

layout (set = 1, binding = 4) readonly buffer Transform {
  SlotUniform slot[];
} u_slots;


// TODO: pass in a uniform
vec4 BG_COLOR = vec4(1);
// vec4 BG_COLOR = vec4(vec3(0), 1);
const vec4 ROW_SEPARATOR_COLOR = vec4(0.86, 0.86, 0.86, 1.0);
const float ROW_SEPARATOR_UV_HEIGHT = 0.035;

float color_map_position(float v) {
  float range = u_color_map.max_val - u_color_map.min_val;
  float v_n = range > 0.0
    ? clamp((v - u_color_map.min_val) / range, 0.0, 1.0)
    : 0.0;

  return clamp(mix(u_color_map.min_color, u_color_map.max_color, v_n), 0.0, 1.0);
}

void main() {
  uint row_offset = i_slot_id * u_data.row_size;

  float t = i_uv.x;

  vec2 ab = u_slots.slot[i_slot_id].ab;
  t = ab.x * t + ab.y;

  float c_t = clamp(t, 0.0, 1.0);

  uint bin_count = u_slots.slot[i_slot_id].bin_count;
  uint data_ix = uint(round(c_t * float(bin_count)));
  data_ix = clamp(data_ix, 0, bin_count - 1);

  float v = u_data.values[row_offset + data_ix];

  vec4 sampled = texture(sampler1D(u_colors, u_sampler), color_map_position(v));

  vec4 color = isinf(v) ? vec4(1.0) : sampled;

  if (i_uv.y <= ROW_SEPARATOR_UV_HEIGHT) {
    color = ROW_SEPARATOR_COLOR;
  }

  f_color = color;

}
