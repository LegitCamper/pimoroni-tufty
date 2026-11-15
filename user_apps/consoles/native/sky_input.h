#pragma once
static void sky_input(sb_emu_state_t* emu, uint32_t buttons, uint64_t seconds) {
    tufty_emu_seconds = seconds;
    memset(&emu->joy, 0, sizeof(emu->joy));
    emu->joy.inputs[SE_KEY_A] = !!(buttons & (1 << 4));
    emu->joy.inputs[SE_KEY_B] = !!(buttons & (1 << 5));
    emu->joy.inputs[SE_KEY_UP] = !!(buttons & 1);
    emu->joy.inputs[SE_KEY_DOWN] = !!(buttons & 2);
    emu->joy.inputs[SE_KEY_LEFT] = !!(buttons & 4);
    emu->joy.inputs[SE_KEY_RIGHT] = !!(buttons & 8);
    emu->joy.inputs[SE_KEY_START] = !!(buttons & ((1 << 6) | (1 << 9)));
    emu->joy.inputs[SE_KEY_SELECT] = !!(buttons & (1 << 10));
    emu->joy.inputs[SE_KEY_L] = !!(buttons & (1 << 7));
    emu->joy.inputs[SE_KEY_R] = !!(buttons & (1 << 8));
    emu->run_mode = SB_MODE_RUN;
    emu->render_frame = true;
    emu->audio_ring_buff.read_ptr = emu->audio_ring_buff.write_ptr;
}
