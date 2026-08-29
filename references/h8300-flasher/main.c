#include <err.h>
#include <inttypes.h>
#include <libusb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define CHECK_ERR(errstr)                    \
    do {                                     \
        if (err < 0) {                       \
            fprintf(stderr, errstr "\n%s\n", \
                    libusb_error_name(err)); \
            libusb_close(dh);                \
            libusb_exit(NULL);               \
            exit(-1);                        \
        }                                    \
    } while(0)

#define PACK __attribute__((__packed__))

#define BULK_EP_IN   0x82
#define BULK_EP_OUT  0x01

#define VENDOR_ID   0x045b // Vendor ID
#define PRODUCT_ID  0x0025 // Product ID

#define BUF_SIZE 64 * 1024 // Max transfer size 64KB

/*****************************************************************************
 * Hitachi BootROM flashing protocol
 *
 * In loop for the whole firmware
 * -> 1024B fimrware data
 * -> 6B CRC
 * <- 1B 0x06 (ACK)
 *
 * -> 1B 0x4B
 * <- 6B CRC (overall?)
 * <- 1B 0x22
 *
 * **************************************************************************/

// Supported device inquiry response
struct PACK dev_inq_hdr_t {
    uint8_t cmd;
    uint8_t size;
    uint8_t ndev;
    uint8_t nchar;
    char code[4];
};

struct PACK dev_sel_t {
    uint8_t cmd;
    uint8_t size;
    char code[4];
    uint8_t sum;
};

struct PACK prog_chunk_t {
    uint8_t cmd;
    uint32_t addr;
    uint8_t data[1024];
    uint8_t sum;
};

struct PACK prog_end_t {
    uint8_t cmd;
    uint32_t addr;
    uint8_t sum;
};

struct PACK sum_chk_t {
    uint8_t cmd;
    uint8_t size;
    uint32_t chk;
    uint8_t sum;
};

uint8_t checksum(uint8_t *data, size_t len) {
    uint8_t sum = 0;
    for (int i = 0; i < len; i++) {
        sum += data[i];
    }
    sum = ~sum;
    sum++;
    return sum;
}

int main(int argc, char *argv[]) {
    struct libusb_device_handle *dh = NULL;

    // Open firmware file
    if(argc < 2) {
        fprintf(stderr, "Error: no binary file provided!\n");
        printf("Usage: ./j8300-flasher BINFILE\n");
        exit(-1);
    }
    FILE *binfile = fopen(argv[1], "rb");
    if (!binfile)
        perror("Error: ");

    // Compute binary file size
    fseek(binfile, 0, SEEK_END);
    long binsize = ftell(binfile);
    fseek(binfile, 0, SEEK_SET);

    // Init libusb
    int err = 0;
    err = libusb_init(NULL);
    CHECK_ERR("cannot initialize libusb!");

    // Open Hitachi USB peripheral
    dh = libusb_open_device_with_vid_pid(NULL,VENDOR_ID,PRODUCT_ID);
    if (!dh)
        fprintf(stderr, "Error: cannot connect to device %d\n", PRODUCT_ID);

    // Read device descriptor
    struct libusb_device_descriptor ddesc = { 0 };
    err = libusb_get_descriptor(dh,
                                LIBUSB_DT_DEVICE,
                                0,
                                (char *) &ddesc,
                                sizeof(ddesc));
    CHECK_ERR("cannot read device descriptor!");

    // Read configuration descriptor
    struct libusb_config_descriptor cdesc = { 0 };
    err = libusb_get_descriptor(dh,
                                LIBUSB_DT_CONFIG,
                                0,
                                (char *) &cdesc,
                                sizeof(cdesc));
    CHECK_ERR("cannot read config descriptor!");

    // Reset device
	err = libusb_reset_device(dh);
    CHECK_ERR("cannot reset device!");

    // Unset auto kernel detach
    err = libusb_set_auto_detach_kernel_driver(dh, 0);
    CHECK_ERR("cannot unset auto-detach!");

    // Detach kernel interface
    if (libusb_kernel_driver_active(dh, 0)) {
        err = libusb_detach_kernel_driver(dh, 0);
        CHECK_ERR("cannot detach kernel!");
    }

    // Set configuration
    err = libusb_set_configuration(dh, 1);
    CHECK_ERR("cannot set configuration!");

    // Claim device
    err = libusb_claim_interface(dh, 0);
    CHECK_ERR("cannot claim interface!");

    // Transfer firmware
    int transferred = 0, received = 0;
    uint8_t buf[BUF_SIZE];
    uint8_t sum = 0;

    // First command     0x55 -> Begin inquiry phase
    char cmd = 0x55;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("cannot begin inquiry phase!");

    // Expected response 0xE6 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               1,
                               &received,
                               0);
    CHECK_ERR("I/O error!");
    if (buf[0] != 0xE6)
        err = -1;
    CHECK_ERR("wrong response from radio!");

    // Second command     0x20 -> Supported Device Inquiry
    cmd = 0x20;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("I/O error!");

    // Expected response  <- Supported Device Response
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               (char *) &buf,
                               sizeof(buf),
                               &received,
                               0);
    // Checksum
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               1,
                               &received,
                               0);
    CHECK_ERR("error in device selection!");
    struct dev_inq_hdr_t *dir = (struct dev_inq_hdr_t *) buf;
    // TODO: Validate checksum
    buf[sizeof(struct dev_inq_hdr_t) + dir->nchar] = '\0';
    printf("Detected radio: %c%c%c%c-%s\n",
           dir->code[0],
           dir->code[1],
           dir->code[2],
           dir->code[3],
           buf + sizeof(struct dev_inq_hdr_t));
    // TODO: Enumerate radios and allow selection

    // Select device to flash
    struct dev_sel_t sel = { 0 };
    sel.cmd = 0x10;
    sel.size = 4;
    for (int i = 0; i < 4; i++)
        sel.code[i] = dir->code[i];
    sel.sum = checksum((uint8_t *) &sel, sizeof(sel) - 1);
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               (char *) &sel,
                               sizeof(sel),
                               &transferred,
                               0);
    CHECK_ERR("error in device selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error in device selection!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error in device selection!");

    // 0x21 -> Clock Mode Inquiry
    cmd = 0x21;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during clock mode inquiry!");
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               (char *) &buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during clock mode inquiry!");
    printf("Supported clock modes:\n");
    for (int i = 0; i < received; i++)
        printf("0x%1X ", buf[i]);
    // Checksum
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               &sum,
                               1,
                               &received,
                               0);
    printf("0x%1X ", sum);
    printf("\n");

    // 0x11 -> Clock Mode Selection
    uint8_t csel[] = { 0x11, 0x01, 0x01, 0xed };
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               (void *) &csel,
                               sizeof(csel),
                               &transferred,
                               0);
    CHECK_ERR("error during clock mode selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error in clock mode selection!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error in clock mode selection!");

    // 0x27 -> Programming Unit Inquiry
    cmd = 0x27;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during programming mode inquiry!");
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               (char *) &buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during programming mode inquiry!");
    printf("Supported programming units:\n");
    for (int i = 0; i < received; i++)
        printf("0x%1X ", buf[i]);
    // Checksum
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               &sum,
                               1,
                               &received,
                               0);
    printf("0x%1X ", sum);
    printf("\n");

    // 0x3F -> New Bit-Rate Selection
    uint8_t bsel[] = { 0x3f, 0x07, 0x04, 0x80, 0x06, 0x40,
                       0x02, 0x01, 0x01, 0xec };
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               (void *) &bsel,
                               sizeof(bsel),
                               &transferred,
                               0);
    CHECK_ERR("error during bit rate selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during bit rate selection!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during bit rate selection!");
    // Bit rate confirmation 0x06 ->
    cmd = 0x06;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during bit rate confirmation!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during bit rate confirmation!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during bit rate confirmation!");

    // Transition to Programming/Erasing State 0x40 ->
    cmd = 0x40;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during transition to programming state!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during transition to programming state!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during transition to programming state!");

    // User MAT Programming Selection 0x43 ->
    cmd = 0x43;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during user MAT programming selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during user MAT programming selection!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during user MAT programming selection!");

    // 128-Byte Programming 0x50 ->
    struct prog_chunk_t c = { 0 };
    c.cmd = 0x50;
    uint32_t bin_sum;
    for(int i = 0; i < binsize / 1024; i++) {
        c.addr = __builtin_bswap32(i * 1024);
        fread(&(c.data), 1, 1024, binfile);
        bin_sum += checksum((uint8_t *)&(c.data), 1024);
        c.sum = checksum((uint8_t *) &c, sizeof(c) - 1);
        err = libusb_bulk_transfer(dh,
                                   BULK_EP_OUT,
                                   (void *) &c,
                                   sizeof(c),
                                   &transferred,
                                   0);
        CHECK_ERR("error during programming!");
        // Expected response 0x06 <- (ACK)
        err = libusb_bulk_transfer(dh,
                                   BULK_EP_IN,
                                   buf,
                                   sizeof(buf),
                                   &received,
                                   0);
        CHECK_ERR("error during programming!");
        if (buf[0] != 0x06)
            err = -1;
        CHECK_ERR("error during programming!");
    }

    // Send 1024 and then last 6

    // Stop Programming Operation
    struct prog_end_t e = { 0 };
    e.cmd = 0x50;
    e.addr = 0xffffffff;
    e.sum = 0xb4;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               (void *) &e,
                               sizeof(e),
                               &transferred,
                               0);
    CHECK_ERR("error during programming stop!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during programming stop!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during programming stop!");

    // User MAT Sum Check 0x4B ->
    cmd = 0x4B;
    err = libusb_bulk_transfer(dh,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during user MAT sum check!");
    err = libusb_bulk_transfer(dh,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during user MAT sum check!");
    struct sum_chk_t *chk = (struct sum_chk_t *) buf;
    if (chk->cmd != 0x5B &&
        chk->size != 4 &&
        chk->sum != checksum((uint8_t *) chk, sizeof(struct sum_chk_t) - 1) &&
        __builtin_bswap32(chk->chk) != bin_sum)
        err = -1;
    CHECK_ERR("error during user MAT sum check!");

    // Terminate transfer
    libusb_close(dh);
    libusb_exit(NULL);
}
