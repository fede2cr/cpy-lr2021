// Native module shim: unwraps CircuitPython objects into integers and calls the
// Rust core.
//
// Natmod only, unlike the LR1121 shim's dual build. That driver is built into
// firmware because its board has ~6 kB of heap headroom; the W12 has 8 MB of
// PSRAM, so a .mpy on the heap costs nothing worth reclaiming.

#include "py/dynruntime.h"

// Bus layout: (spi, cs, busy, ticks_ms, busy_timeout_ms). The module holds no
// state; each call builds the callback table on the stack, which keeps no
// mp_obj_t in BSS where the collector would not scan it.
#define LR2021_BUS_LEN 5

typedef struct _lr2021_ctx_t {
    mp_obj_t spi;
    mp_obj_t cs;
    mp_obj_t busy;
    mp_obj_t ticks;
} lr2021_ctx_t;

// Must match `struct Hal` in ../../lrxxxx-hal/src/hal.rs.
typedef struct _lr2021_hal_t {
    void *ctx;
    int32_t (*spi_write)(void *, const uint8_t *, size_t);
    int32_t (*spi_read)(void *, uint8_t *, size_t);
    int32_t (*cs_set)(void *, int32_t);
    int32_t (*busy_get)(void *);
    int32_t (*ticks_ms)(void *);
    int32_t busy_timeout_ms;
} lr2021_hal_t;

extern int32_t lr2021_flrc_bitrate_kbps(int32_t br_bw);
extern int32_t lr2021_flrc_bandwidth_khz(int32_t br_bw);
extern int32_t lr2021_flrc_modulation_params(int32_t br_bw, int32_t cr, int32_t shaping);
extern int32_t lr2021_flrc_goodput_bps(int32_t bytes, int32_t elapsed_ms);
extern int32_t lr2021_flrc_per_permille(int32_t sent, int32_t received);

extern int32_t lr2021_wait_ready(const lr2021_hal_t *hal);
extern int32_t lr2021_get_status(const lr2021_hal_t *hal, int32_t *out);
extern int32_t lr2021_get_version(const lr2021_hal_t *hal, int32_t *out);
extern int32_t lr2021_get_random_number(const lr2021_hal_t *hal, int32_t *out);
extern int32_t lr2021_get_errors(const lr2021_hal_t *hal);
extern int32_t lr2021_clear_errors(const lr2021_hal_t *hal);
extern int32_t lr2021_set_standby(const lr2021_hal_t *hal, int32_t mode);
extern int32_t lr2021_set_fs(const lr2021_hal_t *hal);
extern int32_t lr2021_set_sleep(const lr2021_hal_t *hal, int32_t config, int32_t wakeup_ticks);
extern int32_t lr2021_activate_pram(const lr2021_hal_t *hal);
extern int32_t lr2021_calibrate(const lr2021_hal_t *hal, int32_t blocks);
extern int32_t lr2021_set_dio_function(const lr2021_hal_t *hal, int32_t dio, int32_t function);
extern int32_t lr2021_set_dio_rf_switch_config(const lr2021_hal_t *hal, int32_t dio, int32_t modes);
extern int32_t lr2021_set_rf_switch(const lr2021_hal_t *hal, int32_t tx_dio, int32_t tx_pull,
    int32_t rx_dio, int32_t rx_pull);

extern int32_t lr2021_set_rf_frequency(const lr2021_hal_t *hal, int32_t hz);
extern int32_t lr2021_set_packet_type(const lr2021_hal_t *hal, int32_t packet_type);
extern int32_t lr2021_get_packet_type(const lr2021_hal_t *hal);
extern int32_t lr2021_set_rx_path(const lr2021_hal_t *hal, int32_t path, int32_t boost);
extern int32_t lr2021_set_power_dbm(const lr2021_hal_t *hal, int32_t dbm, int32_t ramp);
extern int32_t lr2021_set_rx(const lr2021_hal_t *hal, int32_t timeout);
extern int32_t lr2021_set_tx(const lr2021_hal_t *hal, int32_t timeout);
extern int32_t lr2021_set_dio_irq_config(const lr2021_hal_t *hal, int32_t dio, int32_t irq);
extern int32_t lr2021_get_and_clear_irq_status(const lr2021_hal_t *hal);
extern int32_t lr2021_clear_irq(const lr2021_hal_t *hal, int32_t irq);
extern int32_t lr2021_get_rssi_inst(const lr2021_hal_t *hal);
extern int32_t lr2021_write_tx_fifo(const lr2021_hal_t *hal, const uint8_t *data, int32_t len);
extern int32_t lr2021_read_rx_fifo(const lr2021_hal_t *hal, uint8_t *out, int32_t len);
extern int32_t lr2021_get_rx_fifo_level(const lr2021_hal_t *hal);
extern int32_t lr2021_get_tx_fifo_level(const lr2021_hal_t *hal);
extern int32_t lr2021_clear_rx_fifo(const lr2021_hal_t *hal);
extern int32_t lr2021_clear_tx_fifo(const lr2021_hal_t *hal);
extern int32_t lr2021_reset_rx_stats(const lr2021_hal_t *hal);

extern int32_t lr2021_lora_bw_hz(int32_t bw);
extern int32_t lr2021_lora_ldro_needed(int32_t sf, int32_t bw);
extern int32_t lr2021_lora_set_modulation_params(const lr2021_hal_t *hal, int32_t sf, int32_t bw,
    int32_t cr, int32_t ldro, int32_t high_band);
extern int32_t lr2021_lora_set_packet_params(const lr2021_hal_t *hal, int32_t preamble_len,
    int32_t header_type, int32_t payload_len, int32_t crc, int32_t invert_iq);
extern int32_t lr2021_lora_set_sync_word(const lr2021_hal_t *hal, int32_t sync_word);
extern int32_t lr2021_lora_set_synch_timeout(const lr2021_hal_t *hal, int32_t symbols, int32_t mant_exp);
extern int32_t lr2021_lora_get_packet_status(const lr2021_hal_t *hal, int32_t *out);
extern int32_t lr2021_lora_get_rx_stats(const lr2021_hal_t *hal, int32_t *out);
extern int32_t lr2021_lora_set_cad_params(const lr2021_hal_t *hal, int32_t sym_num,
    int32_t preamble_only, int32_t pnr_delta, int32_t exit_mode, int32_t timeout, int32_t det_peak);
extern int32_t lr2021_lora_set_cad(const lr2021_hal_t *hal);

// ---------------------------------------------------------------- callbacks

// The bytearray aliases the caller's buffer instead of copying it. busio.SPI
// does not retain the object, and it dies with the enclosing call.
static int32_t cb_spi_write(void *vctx, const uint8_t *buf, size_t len) {
    lr2021_ctx_t *ctx = (lr2021_ctx_t *)vctx;
    mp_obj_t meth[3];
    mp_load_method(ctx->spi, MP_QSTR_write, meth);
    meth[2] = mp_obj_new_bytearray_by_ref(len, (void *)buf);
    mp_call_function_n_kw(meth[0], 2, 0, &meth[1]);
    return 0;
}

static int32_t cb_spi_read(void *vctx, uint8_t *buf, size_t len) {
    lr2021_ctx_t *ctx = (lr2021_ctx_t *)vctx;
    mp_obj_t meth[3];
    mp_load_method(ctx->spi, MP_QSTR_readinto, meth);
    meth[2] = mp_obj_new_bytearray_by_ref(len, buf);
    mp_call_function_n_kw(meth[0], 2, 0, &meth[1]);
    return 0;
}

static int32_t cb_cs_set(void *vctx, int32_t level) {
    lr2021_ctx_t *ctx = (lr2021_ctx_t *)vctx;
    mp_store_attr(ctx->cs, MP_QSTR_value, level ? mp_const_true : mp_const_false);
    return 0;
}

static int32_t cb_busy_get(void *vctx) {
    lr2021_ctx_t *ctx = (lr2021_ctx_t *)vctx;
    return mp_obj_is_true(mp_load_attr(ctx->busy, MP_QSTR_value)) ? 1 : 0;
}

static int32_t cb_ticks_ms(void *vctx) {
    lr2021_ctx_t *ctx = (lr2021_ctx_t *)vctx;
    return (int32_t)mp_obj_get_int(mp_call_function_n_kw(ctx->ticks, 0, 0, NULL));
}

// ------------------------------------------------------------------ helpers

static mp_obj_t bus_item(mp_obj_t bus, mp_int_t i) {
    return mp_obj_subscr(bus, MP_OBJ_NEW_SMALL_INT(i), MP_OBJ_SENTINEL);
}

static void bus_bind(mp_obj_t bus, lr2021_ctx_t *ctx, lr2021_hal_t *hal) {
    if (mp_obj_get_int(mp_obj_len(bus)) != LR2021_BUS_LEN) {
        mp_raise_ValueError(MP_ERROR_TEXT("bus must be (spi, cs, busy, ticks_ms, timeout_ms)"));
    }
    ctx->spi = bus_item(bus, 0);
    ctx->cs = bus_item(bus, 1);
    ctx->busy = bus_item(bus, 2);
    ctx->ticks = bus_item(bus, 3);

    hal->ctx = ctx;
    hal->spi_write = cb_spi_write;
    hal->spi_read = cb_spi_read;
    hal->cs_set = cb_cs_set;
    hal->busy_get = cb_busy_get;
    hal->ticks_ms = cb_ticks_ms;
    hal->busy_timeout_ms = (int32_t)mp_obj_get_int(bus_item(bus, 4));
}

// Binds the bus and declares `hal`, so each wrapper below stays one line of work.
#define LR2021_BIND(bus)         \
    lr2021_ctx_t ctx;            \
    lr2021_hal_t hal;            \
    bus_bind((bus), &ctx, &hal)

static void check_err(int32_t rc) {
    if (rc >= 0) {
        return;
    }
    switch (rc) {
        case -1:
            mp_raise_msg(&mp_type_RuntimeError, MP_ERROR_TEXT("LR2021 SPI or GPIO call failed"));
        case -2:
            mp_raise_msg(&mp_type_RuntimeError, MP_ERROR_TEXT("LR2021 BUSY stayed high"));
        case -3:
            mp_raise_msg(&mp_type_RuntimeError, MP_ERROR_TEXT("LR2021 could not execute the command"));
        case -4:
            mp_raise_msg(&mp_type_RuntimeError, MP_ERROR_TEXT("LR2021 reported a processing error"));
        case -5:
            mp_raise_ValueError(MP_ERROR_TEXT("buffer too long for this command"));
        case -6:
            mp_raise_ValueError(MP_ERROR_TEXT("bus entry missing or null"));
        case -8:
            mp_raise_msg(&mp_type_RuntimeError, MP_ERROR_TEXT("no answer from the LR2021: check power, MISO and NRESET"));
        case -9:
            mp_raise_ValueError(MP_ERROR_TEXT("argument out of range for this command"));
        default:
            mp_raise_msg(&mp_type_RuntimeError, MP_ERROR_TEXT("lr2021 error"));
    }
}

static mp_obj_t mod_flrc_bitrate_kbps(mp_obj_t br_bw) {
    int32_t rc = lr2021_flrc_bitrate_kbps(mp_obj_get_int(br_bw));
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_flrc_bitrate_kbps_obj, mod_flrc_bitrate_kbps);

static mp_obj_t mod_flrc_bandwidth_khz(mp_obj_t br_bw) {
    int32_t rc = lr2021_flrc_bandwidth_khz(mp_obj_get_int(br_bw));
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_flrc_bandwidth_khz_obj, mod_flrc_bandwidth_khz);

static mp_obj_t mod_flrc_modulation_params(mp_obj_t br_bw, mp_obj_t cr, mp_obj_t shaping) {
    int32_t rc = lr2021_flrc_modulation_params(
        mp_obj_get_int(br_bw), mp_obj_get_int(cr), mp_obj_get_int(shaping));
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_flrc_modulation_params_obj, mod_flrc_modulation_params);

static mp_obj_t mod_flrc_goodput_bps(mp_obj_t bytes, mp_obj_t elapsed_ms) {
    int32_t rc = lr2021_flrc_goodput_bps(mp_obj_get_int(bytes), mp_obj_get_int(elapsed_ms));
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_flrc_goodput_bps_obj, mod_flrc_goodput_bps);

static mp_obj_t mod_flrc_per_permille(mp_obj_t sent, mp_obj_t received) {
    int32_t rc = lr2021_flrc_per_permille(mp_obj_get_int(sent), mp_obj_get_int(received));
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_flrc_per_permille_obj, mod_flrc_per_permille);

// The edge is a parameter rather than a constant as in the LR1121 shim: the
// radio's DIO8 rises, but the only pin on this board that can be triggered by
// hand is the BOOT button, which falls.
static mp_obj_t mod_attach_irq(mp_obj_t pin, mp_obj_t handler, mp_obj_t edge) {
    if (!mp_pin_interrupt_attach(pin, mp_obj_get_int(edge), handler, pin)) {
        mp_raise_msg(&mp_type_RuntimeError,
            MP_ERROR_TEXT("cannot watch that pin: no port support, already in use, or no slot left"));
    }
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_attach_irq_obj, mod_attach_irq);

static mp_obj_t mod_detach_irq(mp_obj_t pin) {
    mp_pin_interrupt_detach(pin);
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_detach_irq_obj, mod_detach_irq);

// -------------------------------------------------------------------- system

static mp_obj_t mod_wait_ready(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_wait_ready(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_wait_ready_obj, mod_wait_ready);

static mp_obj_t mod_get_status(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t out[3];
    check_err(lr2021_get_status(&hal, out));
    mp_obj_t items[3] = { mp_obj_new_int(out[0]), mp_obj_new_int(out[1]), mp_obj_new_int(out[2]) };
    return mp_obj_new_tuple(3, items);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_status_obj, mod_get_status);

static mp_obj_t mod_get_version(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t out[2];
    check_err(lr2021_get_version(&hal, out));
    mp_obj_t items[2] = { mp_obj_new_int(out[0]), mp_obj_new_int(out[1]) };
    return mp_obj_new_tuple(2, items);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_version_obj, mod_get_version);

static mp_obj_t mod_get_random_number(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t out;
    check_err(lr2021_get_random_number(&hal, &out));
    return mp_obj_new_int_from_uint((uint32_t)out);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_random_number_obj, mod_get_random_number);

static mp_obj_t mod_get_errors(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t rc = lr2021_get_errors(&hal);
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_errors_obj, mod_get_errors);

static mp_obj_t mod_clear_errors(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_clear_errors(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_clear_errors_obj, mod_clear_errors);

static mp_obj_t mod_set_standby(mp_obj_t bus, mp_obj_t mode_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_standby(&hal, mp_obj_get_int(mode_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_set_standby_obj, mod_set_standby);

static mp_obj_t mod_set_fs(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_set_fs(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_set_fs_obj, mod_set_fs);

static mp_obj_t mod_set_sleep(mp_obj_t bus, mp_obj_t config_in, mp_obj_t ticks_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_sleep(&hal, mp_obj_get_int(config_in), mp_obj_get_int(ticks_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_set_sleep_obj, mod_set_sleep);

static mp_obj_t mod_activate_pram(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_activate_pram(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_activate_pram_obj, mod_activate_pram);

static mp_obj_t mod_calibrate(mp_obj_t bus, mp_obj_t blocks_in) {
    LR2021_BIND(bus);
    check_err(lr2021_calibrate(&hal, mp_obj_get_int(blocks_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_calibrate_obj, mod_calibrate);

static mp_obj_t mod_set_dio_function(mp_obj_t bus, mp_obj_t dio_in, mp_obj_t function_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_dio_function(&hal, mp_obj_get_int(dio_in), mp_obj_get_int(function_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_set_dio_function_obj, mod_set_dio_function);

static mp_obj_t mod_set_dio_rf_switch_config(mp_obj_t bus, mp_obj_t dio_in, mp_obj_t modes_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_dio_rf_switch_config(&hal, mp_obj_get_int(dio_in), mp_obj_get_int(modes_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_set_dio_rf_switch_config_obj, mod_set_dio_rf_switch_config);

// Two-line front ends only. A three-line switch such as the W12's sub-GHz
// GC1109 goes through set_dio_function and set_dio_rf_switch_config directly.
static mp_obj_t mod_set_rf_switch(size_t n_args, const mp_obj_t *args) {
    LR2021_BIND(args[0]);
    check_err(lr2021_set_rf_switch(&hal, mp_obj_get_int(args[1]), mp_obj_get_int(args[2]),
        mp_obj_get_int(args[3]), mp_obj_get_int(args[4])));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_VAR_BETWEEN(mod_set_rf_switch_obj, 5, 5, mod_set_rf_switch);

// --------------------------------------------------------------------- radio

static mp_obj_t mod_set_rf_frequency(mp_obj_t bus, mp_obj_t hz_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_rf_frequency(&hal, mp_obj_get_int(hz_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_set_rf_frequency_obj, mod_set_rf_frequency);

static mp_obj_t mod_set_packet_type(mp_obj_t bus, mp_obj_t type_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_packet_type(&hal, mp_obj_get_int(type_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_set_packet_type_obj, mod_set_packet_type);

static mp_obj_t mod_get_packet_type(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t rc = lr2021_get_packet_type(&hal);
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_packet_type_obj, mod_get_packet_type);

static mp_obj_t mod_set_rx_path(mp_obj_t bus, mp_obj_t path_in, mp_obj_t boost_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_rx_path(&hal, mp_obj_get_int(path_in), mp_obj_get_int(boost_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_set_rx_path_obj, mod_set_rx_path);

static mp_obj_t mod_set_power_dbm(mp_obj_t bus, mp_obj_t dbm_in, mp_obj_t ramp_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_power_dbm(&hal, mp_obj_get_int(dbm_in), mp_obj_get_int(ramp_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_set_power_dbm_obj, mod_set_power_dbm);

static mp_obj_t mod_set_rx(mp_obj_t bus, mp_obj_t timeout_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_rx(&hal, mp_obj_get_int(timeout_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_set_rx_obj, mod_set_rx);

static mp_obj_t mod_set_tx(mp_obj_t bus, mp_obj_t timeout_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_tx(&hal, mp_obj_get_int(timeout_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_set_tx_obj, mod_set_tx);

static mp_obj_t mod_set_dio_irq_config(mp_obj_t bus, mp_obj_t dio_in, mp_obj_t irq_in) {
    LR2021_BIND(bus);
    check_err(lr2021_set_dio_irq_config(&hal, mp_obj_get_int(dio_in),
        mp_obj_get_int_truncated(irq_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_set_dio_irq_config_obj, mod_set_dio_irq_config);

static mp_obj_t mod_get_and_clear_irq_status(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t rc = lr2021_get_and_clear_irq_status(&hal);
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_and_clear_irq_status_obj, mod_get_and_clear_irq_status);

static mp_obj_t mod_clear_irq(mp_obj_t bus, mp_obj_t irq_in) {
    LR2021_BIND(bus);
    check_err(lr2021_clear_irq(&hal, mp_obj_get_int_truncated(irq_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_clear_irq_obj, mod_clear_irq);

static mp_obj_t mod_get_rssi_inst(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t rc = lr2021_get_rssi_inst(&hal);
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_rssi_inst_obj, mod_get_rssi_inst);

static mp_obj_t mod_write_tx_fifo(mp_obj_t bus, mp_obj_t data_in) {
    LR2021_BIND(bus);
    mp_buffer_info_t data;
    mp_get_buffer_raise(data_in, &data, MP_BUFFER_READ);
    check_err(lr2021_write_tx_fifo(&hal, (const uint8_t *)data.buf, (int32_t)data.len));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_write_tx_fifo_obj, mod_write_tx_fifo);

// Returns bytes rather than filling a caller buffer: the length is only known
// after get_rx_fifo_level, so there is nothing to preallocate.
static mp_obj_t mod_read_rx_fifo(mp_obj_t bus, mp_obj_t len_in) {
    LR2021_BIND(bus);
    mp_int_t len = mp_obj_get_int(len_in);
    if (len < 0 || len > 255) {
        mp_raise_ValueError(MP_ERROR_TEXT("length must be 0..255"));
    }
    uint8_t buf[255];
    check_err(lr2021_read_rx_fifo(&hal, buf, (int32_t)len));
    return mp_obj_new_bytes(buf, (size_t)len);
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_read_rx_fifo_obj, mod_read_rx_fifo);

static mp_obj_t mod_get_rx_fifo_level(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t rc = lr2021_get_rx_fifo_level(&hal);
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_rx_fifo_level_obj, mod_get_rx_fifo_level);

static mp_obj_t mod_get_tx_fifo_level(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t rc = lr2021_get_tx_fifo_level(&hal);
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_get_tx_fifo_level_obj, mod_get_tx_fifo_level);

static mp_obj_t mod_clear_rx_fifo(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_clear_rx_fifo(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_clear_rx_fifo_obj, mod_clear_rx_fifo);

static mp_obj_t mod_clear_tx_fifo(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_clear_tx_fifo(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_clear_tx_fifo_obj, mod_clear_tx_fifo);

static mp_obj_t mod_reset_rx_stats(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_reset_rx_stats(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_reset_rx_stats_obj, mod_reset_rx_stats);

// ---------------------------------------------------------------------- LoRa

static mp_obj_t mod_lora_bw_hz(mp_obj_t bw_in) {
    int32_t rc = lr2021_lora_bw_hz(mp_obj_get_int(bw_in));
    check_err(rc);
    return mp_obj_new_int(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_lora_bw_hz_obj, mod_lora_bw_hz);

static mp_obj_t mod_lora_ldro_needed(mp_obj_t sf_in, mp_obj_t bw_in) {
    int32_t rc = lr2021_lora_ldro_needed(mp_obj_get_int(sf_in), mp_obj_get_int(bw_in));
    check_err(rc);
    return mp_obj_new_bool(rc);
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_lora_ldro_needed_obj, mod_lora_ldro_needed);

static mp_obj_t mod_lora_set_modulation_params(size_t n_args, const mp_obj_t *args) {
    LR2021_BIND(args[0]);
    check_err(lr2021_lora_set_modulation_params(&hal, mp_obj_get_int(args[1]),
        mp_obj_get_int(args[2]), mp_obj_get_int(args[3]), mp_obj_get_int(args[4]),
        mp_obj_is_true(args[5])));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_VAR_BETWEEN(mod_lora_set_modulation_params_obj, 6, 6,
    mod_lora_set_modulation_params);

static mp_obj_t mod_lora_set_packet_params(size_t n_args, const mp_obj_t *args) {
    LR2021_BIND(args[0]);
    check_err(lr2021_lora_set_packet_params(&hal, mp_obj_get_int(args[1]),
        mp_obj_get_int(args[2]), mp_obj_get_int(args[3]), mp_obj_is_true(args[4]),
        mp_obj_is_true(args[5])));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_VAR_BETWEEN(mod_lora_set_packet_params_obj, 6, 6,
    mod_lora_set_packet_params);

static mp_obj_t mod_lora_set_sync_word(mp_obj_t bus, mp_obj_t sw_in) {
    LR2021_BIND(bus);
    check_err(lr2021_lora_set_sync_word(&hal, mp_obj_get_int(sw_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(mod_lora_set_sync_word_obj, mod_lora_set_sync_word);

static mp_obj_t mod_lora_set_synch_timeout(mp_obj_t bus, mp_obj_t symbols_in, mp_obj_t mant_exp_in) {
    LR2021_BIND(bus);
    check_err(lr2021_lora_set_synch_timeout(&hal, mp_obj_get_int(symbols_in),
        mp_obj_is_true(mant_exp_in)));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_3(mod_lora_set_synch_timeout_obj, mod_lora_set_synch_timeout);

// RSSI and SNR stay in register units: dBm is -rssi / 2 and dB is snr / 4.
// Scaling them here would force a float across a boundary that has none.
static mp_obj_t mod_lora_get_packet_status(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t out[7];
    check_err(lr2021_lora_get_packet_status(&hal, out));
    mp_obj_t items[7] = {
        mp_obj_new_int(out[0]), mp_obj_new_bool(out[1]), mp_obj_new_int(out[2]),
        mp_obj_new_int(out[3]), mp_obj_new_int(out[4]), mp_obj_new_int(out[5]),
        mp_obj_new_int(out[6]),
    };
    return mp_obj_new_tuple(7, items);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_lora_get_packet_status_obj, mod_lora_get_packet_status);

static mp_obj_t mod_lora_get_rx_stats(mp_obj_t bus) {
    LR2021_BIND(bus);
    int32_t out[4];
    check_err(lr2021_lora_get_rx_stats(&hal, out));
    mp_obj_t items[4] = {
        mp_obj_new_int(out[0]), mp_obj_new_int(out[1]),
        mp_obj_new_int(out[2]), mp_obj_new_int(out[3]),
    };
    return mp_obj_new_tuple(4, items);
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_lora_get_rx_stats_obj, mod_lora_get_rx_stats);

static mp_obj_t mod_lora_set_cad_params(size_t n_args, const mp_obj_t *args) {
    LR2021_BIND(args[0]);
    check_err(lr2021_lora_set_cad_params(&hal, mp_obj_get_int(args[1]),
        mp_obj_is_true(args[2]), mp_obj_get_int(args[3]), mp_obj_get_int(args[4]),
        mp_obj_get_int(args[5]), mp_obj_get_int(args[6])));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_VAR_BETWEEN(mod_lora_set_cad_params_obj, 7, 7, mod_lora_set_cad_params);

static mp_obj_t mod_lora_set_cad(mp_obj_t bus) {
    LR2021_BIND(bus);
    check_err(lr2021_lora_set_cad(&hal));
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(mod_lora_set_cad_obj, mod_lora_set_cad);

// Every export is listed once and expanded twice: as an object definition and
// as a registration. The qstr must be a literal argument -- mpy_ld.py collects
// them with a plain regex over raw source, so MP_QSTR_##name is invisible to it.
#define LR2021_EXPORTS(FUN) \
    FUN(MP_QSTR_flrc_bitrate_kbps, mod_flrc_bitrate_kbps_obj) \
    FUN(MP_QSTR_flrc_bandwidth_khz, mod_flrc_bandwidth_khz_obj) \
    FUN(MP_QSTR_flrc_modulation_params, mod_flrc_modulation_params_obj) \
    FUN(MP_QSTR_flrc_goodput_bps, mod_flrc_goodput_bps_obj) \
    FUN(MP_QSTR_flrc_per_permille, mod_flrc_per_permille_obj) \
    FUN(MP_QSTR_attach_irq, mod_attach_irq_obj) \
    FUN(MP_QSTR_detach_irq, mod_detach_irq_obj) \
    \
    FUN(MP_QSTR_wait_ready, mod_wait_ready_obj) \
    FUN(MP_QSTR_get_status, mod_get_status_obj) \
    FUN(MP_QSTR_get_version, mod_get_version_obj) \
    FUN(MP_QSTR_get_random_number, mod_get_random_number_obj) \
    FUN(MP_QSTR_get_errors, mod_get_errors_obj) \
    FUN(MP_QSTR_clear_errors, mod_clear_errors_obj) \
    FUN(MP_QSTR_set_standby, mod_set_standby_obj) \
    FUN(MP_QSTR_set_fs, mod_set_fs_obj) \
    FUN(MP_QSTR_set_sleep, mod_set_sleep_obj) \
    FUN(MP_QSTR_activate_pram, mod_activate_pram_obj) \
    FUN(MP_QSTR_calibrate, mod_calibrate_obj) \
    FUN(MP_QSTR_set_dio_function, mod_set_dio_function_obj) \
    FUN(MP_QSTR_set_dio_rf_switch_config, mod_set_dio_rf_switch_config_obj) \
    FUN(MP_QSTR_set_rf_switch, mod_set_rf_switch_obj) \
    \
    FUN(MP_QSTR_set_rf_frequency, mod_set_rf_frequency_obj) \
    FUN(MP_QSTR_set_packet_type, mod_set_packet_type_obj) \
    FUN(MP_QSTR_get_packet_type, mod_get_packet_type_obj) \
    FUN(MP_QSTR_set_rx_path, mod_set_rx_path_obj) \
    FUN(MP_QSTR_set_power_dbm, mod_set_power_dbm_obj) \
    FUN(MP_QSTR_set_rx, mod_set_rx_obj) \
    FUN(MP_QSTR_set_tx, mod_set_tx_obj) \
    FUN(MP_QSTR_set_dio_irq_config, mod_set_dio_irq_config_obj) \
    FUN(MP_QSTR_get_and_clear_irq_status, mod_get_and_clear_irq_status_obj) \
    FUN(MP_QSTR_clear_irq, mod_clear_irq_obj) \
    FUN(MP_QSTR_get_rssi_inst, mod_get_rssi_inst_obj) \
    FUN(MP_QSTR_write_tx_fifo, mod_write_tx_fifo_obj) \
    FUN(MP_QSTR_read_rx_fifo, mod_read_rx_fifo_obj) \
    FUN(MP_QSTR_get_rx_fifo_level, mod_get_rx_fifo_level_obj) \
    FUN(MP_QSTR_get_tx_fifo_level, mod_get_tx_fifo_level_obj) \
    FUN(MP_QSTR_clear_rx_fifo, mod_clear_rx_fifo_obj) \
    FUN(MP_QSTR_clear_tx_fifo, mod_clear_tx_fifo_obj) \
    FUN(MP_QSTR_reset_rx_stats, mod_reset_rx_stats_obj) \
    \
    FUN(MP_QSTR_lora_bw_hz, mod_lora_bw_hz_obj) \
    FUN(MP_QSTR_lora_ldro_needed, mod_lora_ldro_needed_obj) \
    FUN(MP_QSTR_lora_set_modulation_params, mod_lora_set_modulation_params_obj) \
    FUN(MP_QSTR_lora_set_packet_params, mod_lora_set_packet_params_obj) \
    FUN(MP_QSTR_lora_set_sync_word, mod_lora_set_sync_word_obj) \
    FUN(MP_QSTR_lora_set_synch_timeout, mod_lora_set_synch_timeout_obj) \
    FUN(MP_QSTR_lora_get_packet_status, mod_lora_get_packet_status_obj) \
    FUN(MP_QSTR_lora_get_rx_stats, mod_lora_get_rx_stats_obj) \
    FUN(MP_QSTR_lora_set_cad_params, mod_lora_set_cad_params_obj) \
    FUN(MP_QSTR_lora_set_cad, mod_lora_set_cad_obj)

#define LR2021_REGISTER(qstr, obj) mp_store_global(qstr, MP_OBJ_FROM_PTR(&obj));

mp_obj_t mpy_init(mp_obj_fun_bc_t *self, size_t n_args, size_t n_kw, mp_obj_t *args) {
    MP_DYNRUNTIME_INIT_ENTRY
    LR2021_EXPORTS(LR2021_REGISTER)
    mp_store_global(MP_QSTR_IRQ_RISING, MP_OBJ_NEW_SMALL_INT(MP_PIN_INTERRUPT_RISING));
    mp_store_global(MP_QSTR_IRQ_FALLING, MP_OBJ_NEW_SMALL_INT(MP_PIN_INTERRUPT_FALLING));
    mp_store_global(MP_QSTR_IRQ_BOTH, MP_OBJ_NEW_SMALL_INT(MP_PIN_INTERRUPT_BOTH));
    MP_DYNRUNTIME_INIT_EXIT
}
