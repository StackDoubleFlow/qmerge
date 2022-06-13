#pragma once
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  void* fnptr;
  size_t method_ref_idx;
} func_lut_entry_t;

void *merge_codegen_resolve_method(const char *mod_id, size_t method_ref_idx);
void *merge_codegen_initialize_method(const char *mod_id, size_t metadata_usage_idx);
void merge_prestub();

#ifdef __cplusplus
}
#endif
