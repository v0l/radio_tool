/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 v0l <radio_tool@v0l.io>
 *
 * radio_tool is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * radio_tool is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with radio_tool. If not, see <https://www.gnu.org/licenses/>.
 */
#include <radio_tool/h8sx/h8sx.hpp>

#include <algorithm>
#include <chrono>
#include <cstring>
#include <exception>
#include <thread>

#if defined(_MSC_VER)
#define bswap32(x) _byteswap_ulong((x))
#else
#define bswap32(x) __builtin_bswap32((x))
#endif

using namespace radio_tool::h8sx;

auto H8SX::IdentifyDevice() -> std::string
{
    int err = 0;
    int transferred = 0, received = 0;
    uint8_t buf[BUF_SIZE];

    // First command     0x55 -> Begin inquiry phase
    uint8_t cmd = static_cast<uint8_t>(H8SXCmd::BEGIN_INQUIRY);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("cannot begin inquiry phase!");

    // Expected response 0xE6 <- (ACK)
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("I/O error!");
    if (buf[0] != 0xE6)
        err = -1;
    CHECK_ERR("wrong response from radio!");

    // Second command     0x20 -> Supported Device Inquiry
    cmd = static_cast<uint8_t>(H8SXCmd::DEVICE_INQUIRY);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("I/O error!");

    // Expected response  <- Supported Device Response
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    // Checksum
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               buf,
                               1,
                               &received,
                               0);
    CHECK_ERR("error in device selection!");
    dir = (struct dev_inq_hdr_t *)buf;
    // TODO: Validate checksum
    buf[sizeof(struct dev_inq_hdr_t) + dir->nchar] = '\0';

    // Return device identifier
    std::ostringstream dev_str;
    dev_str << dir->code[0]
            << dir->code[1]
            << dir->code[2]
            << dir->code[3]
            << "-"
            << buf + sizeof(struct dev_inq_hdr_t);
    return dev_str.str();
}

auto H8SX::Download(const std::vector<uint8_t> &data) const -> void
{
    int err = 0;
    int transferred = 0, received = 0;
    uint8_t buf[BUF_SIZE];

    InitDownload();

    // 128-Byte Programming 0x50 ->
    struct prog_chunk_t c = {};
    uint8_t cmd = static_cast<uint8_t>(H8SXCmd::PROGRAM_128B);
    uint32_t bin_sum = 0;
    for (std::vector<uint8_t>::size_type i = 0; i < data.size() / 1024; i++)
    {
        c.addr = bswap32(i * 1024);
        std::copy(data.begin() + i * 1024, data.begin() + (i + 1) * 1024, c.data);
        bin_sum += Checksum((uint8_t *)&(c.data), 1024);
        c.sum = Checksum((uint8_t *)&c, sizeof(c) - 1);
        err = libusb_bulk_transfer(device,
                                   BULK_EP_OUT,
                                   (uint8_t *)&c,
                                   sizeof(c),
                                   &transferred,
                                   0);
        CHECK_ERR("error during programming!");
        // Expected response 0x06 <- (ACK)
        err = libusb_bulk_transfer(device,
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
    struct prog_end_t e = {};
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               (uint8_t *)&e,
                               sizeof(e),
                               &transferred,
                               0);
    CHECK_ERR("error during programming stop!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
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
    cmd = static_cast<uint8_t>(H8SXCmd::USER_MAT_CHECKSUM);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during user MAT sum check!");
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during user MAT sum check!");
    struct sum_chk_t *chk = (struct sum_chk_t *)buf;
    if (chk->cmd != 0x5B &&
        chk->size != 4 &&
        chk->sum != Checksum((uint8_t *)chk, sizeof(struct sum_chk_t) - 1) &&
        bswap32(chk->chk) != bin_sum)
        err = -1;
    CHECK_ERR("error during user MAT sum check!");
}

auto H8SX::InitDownload() const -> void
{
    int err = 0;
    int transferred = 0, received = 0;
    uint8_t buf[BUF_SIZE];
    uint8_t sum = 0;

    // Select device to flash
    struct dev_sel_t sel = {0};
    sel.cmd = static_cast<uint8_t>(H8SXCmd::DEVICE_SELECT);
    sel.size = 4;
    for (int i = 0; i < 4; i++)
        sel.code[i] = dir->code[i];
    sel.sum = Checksum((uint8_t *)&sel, sizeof(sel) - 1);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               (uint8_t *)&sel,
                               sizeof(sel),
                               &transferred,
                               0);
    CHECK_ERR("error in device selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
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
    uint8_t cmd = static_cast<uint8_t>(H8SXCmd::CLOCK_MODE_INQUIRY);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during clock mode inquiry!");
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               (uint8_t *)&buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during clock mode inquiry!");
    // Checksum
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               &sum,
                               1,
                               &received,
                               0);

    // 0x11 -> Clock Mode Selection
    uint8_t csel[] = {0x11, 0x01, 0x01, 0xed};
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               (uint8_t *)&csel,
                               sizeof(csel),
                               &transferred,
                               0);
    CHECK_ERR("error during clock mode selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
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
    cmd = static_cast<uint8_t>(H8SXCmd::PROG_UNIT_INQUIRY);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during programming mode inquiry!");
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               (uint8_t *)&buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during programming mode inquiry!");
    // Checksum
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               &sum,
                               1,
                               &received,
                               0);

    // 0x3F -> New Bit-Rate Selection
    uint8_t bsel[] = {0x3f, 0x07, 0x04, 0x80, 0x06, 0x40,
                      0x02, 0x01, 0x01, 0xec};
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               (uint8_t *)&bsel,
                               sizeof(bsel),
                               &transferred,
                               0);
    CHECK_ERR("error during bit rate selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
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
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during bit rate confirmation!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
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
    cmd = static_cast<uint8_t>(H8SXCmd::BEGIN_PROGRAMMING);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during transition to programming state!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
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
    cmd = static_cast<uint8_t>(H8SXCmd::USER_MAT_SELECT);
    err = libusb_bulk_transfer(device,
                               BULK_EP_OUT,
                               &cmd,
                               1,
                               &transferred,
                               0);
    CHECK_ERR("error during user MAT programming selection!");
    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device,
                               BULK_EP_IN,
                               buf,
                               sizeof(buf),
                               &received,
                               0);
    CHECK_ERR("error during user MAT programming selection!");
    if (buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during user MAT programming selection!");
}

auto H8SX::Init() const -> void
{
    int err = 0;

    // Reset device
    err = libusb_reset_device(device);
    CHECK_ERR("cannot reset device!");

    // Unset auto kernel detach
    err = libusb_set_auto_detach_kernel_driver(device, 0);
    CHECK_ERR("cannot unset auto-detach!");

    // Detach kernel interface
    if (libusb_kernel_driver_active(device, 0))
    {
        err = libusb_detach_kernel_driver(device, 0);
        CHECK_ERR("cannot detach kernel!");
    }

    // Set configuration
    err = libusb_set_configuration(device, 1);
    CHECK_ERR("cannot set configuration!");

    // Claim device
    err = libusb_claim_interface(device, 0);
    CHECK_ERR("cannot claim interface!");
}

auto H8SX::CheckDevice() const -> void
{
    if (this->device == nullptr)
        throw std::runtime_error("Device is not opened");
}

auto H8SX::Checksum(const uint8_t *data, size_t len) const -> uint8_t
{
    uint8_t sum = 0;
    for (size_t i = 0; i < len; i++)
    {
        sum += data[i];
    }
    sum = ~sum;
    sum++;
    return sum;
}
