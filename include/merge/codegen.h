#pragma once
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

void *merge_codegen_resolve_method(const char *mod_id, size_t method_ref_idx);

#ifdef __cplusplus
}
#endif
