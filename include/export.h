#ifndef _RADIO_TOOL_H
#define _RADIO_TOOL_H

struct radiotool_ctx;
typedef struct radiotool_ctx* radiotool_ctx_ptr;

typedef struct
{
    const char* manufacturer;
    const char* model;
    const char* port;
} radiotool_radio_info;

typedef struct
{
    const char* model;
} radiotool_firmware_info;

enum radiotool_status
{
    RDTS_FIRMWARE_NOT_SUPPORTED = -2,
    RDTS_INVALID_CTX = -1,
    RDTS_OK = 0
};

/**
 * Initialize ctx structure
 */
int radiotool_init(radiotool_ctx_ptr *ctx);

/**
 * Close context and free resources
 */
int radiotool_close(radiotool_ctx_ptr ctx);

/**
 * List devices
 * Returns error or number of devices
 */
int radiotool_list_devices(radiotool_ctx_ptr ctx, radiotool_radio_info **infos);

/** 
 * Free radio info array information
 */
int radiotool_free_radio_infos(radiotool_radio_info *infos, int n);

/**
 * Get firmware info about a file
 */
int radiotool_load_firmware(radiotool_ctx_ptr ctx, const char *file, radiotool_firmware_info **info);

#endif