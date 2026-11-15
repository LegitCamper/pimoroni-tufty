#ifndef TUFTY_LUA_CONFIG_H
#define TUFTY_LUA_CONFIG_H
#define LUA_32BITS 1
#define LUAI_MAXCCALLS 40
#define luai_makeseed(state) 0x74554654u
#define l_randomizePivot() 0x74554654u
#endif
