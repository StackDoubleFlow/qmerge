#pragma once
#include <stddef.h>

void *merge_codegen_resolve_method(const char *mod_id, size_t method_ref_idx);
void *merge_codegen_initialize_method(const char *mod_id, size_t metadata_usage_idx);
