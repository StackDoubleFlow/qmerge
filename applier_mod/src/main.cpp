#include "MergeLogger.h"

// Called at the early stages of game loading
extern "C" void setup(ModInfo& info) {
    info.id = MOD_ID;
    info.version = VERSION;
}

extern "C" void load() {}