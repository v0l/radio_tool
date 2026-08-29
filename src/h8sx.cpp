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
#include <cstdlib>
#include <cstring>
#include <exception>
#include <memory>
#include <thread>
#include "radio_tool/util.hpp"

using namespace radio_tool::h8sx;

auto H8SX::IdentifyDevice() const -> std::string
{
    struct dev_inq_hdr_t *dir = nullptr;
    InquireDevice(&dir);

    // Return device identifier
    std::ostringstream dev_str;
    dev_str << std::string(dir->code, dir->code + sizeof(dir->code))
            << "-"
            << std::string((char *)dir + sizeof(struct dev_inq_hdr_t), dir->nchar);

    free(dir);
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
    uint32_t bin_sum = 0;
    constexpr size_t ChunkSize = sizeof(c.data);

    //the tail of the binary is sent in a final short chunk padded with 0xff,
    //previously anything after the last whole chunk was silently dropped
    auto chunks = (data.size() + ChunkSize - 1) / ChunkSize;
    for (size_t i = 0; i < chunks; i++)
    {
        auto offset = i * ChunkSize;
        auto count = std::min(ChunkSize, data.size() - offset);

        c.cmd = static_cast<uint8_t>(H8SXCmd::PROGRAM_128B);
        c.addr = bswap32((uint32_t)offset);
        std::fill(c.data, c.data + ChunkSize, 0xff);
        std::copy(data.begin() + offset, data.begin() + offset + count, c.data);
        bin_sum += Checksum((uint8_t *)&(c.data), ChunkSize);
        c.sum = Checksum((uint8_t *)&c, sizeof(c) - 1);
        err = libusb_bulk_transfer(device, BULK_EP_OUT, (uint8_t *)&c, sizeof(c), &transferred, 0);
        CHECK_ERR("error during programming!");

        // Expected response 0x06 <- (ACK)
        err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
        CHECK_ERR("error during programming!");
        if (received < 1 || buf[0] != 0x06)
            err = -1;
        CHECK_ERR("error during programming!");
    }

    // Stop Programming Operation
    struct prog_end_t e = {};
    err = libusb_bulk_transfer(device, BULK_EP_OUT, (uint8_t *)&e, sizeof(e), &transferred, 0);
    CHECK_ERR("error during programming stop!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during programming stop!");
    if (received < 1 || buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during programming stop!");

    // User MAT Sum Check 0x4B ->
    uint8_t cmd = static_cast<uint8_t>(H8SXCmd::USER_MAT_CHECKSUM);
    err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
    CHECK_ERR("error during user MAT sum check!");
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during user MAT sum check!");

    //any one of these being wrong is a failure, joining them with && meant
    //the check could never fail
    struct sum_chk_t *chk = (struct sum_chk_t *)buf;
    if ((size_t)received < sizeof(struct sum_chk_t) ||
        chk->cmd != 0x5B ||
        chk->size != 4 ||
        chk->sum != Checksum((uint8_t *)chk, sizeof(struct sum_chk_t) - 1) ||
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

    struct dev_inq_hdr_t *dir = nullptr;
    InquireDevice(&dir);

    //the inquiry buffer is malloc'd, and CHECK_ERR throws, so it has to be
    //released on the way out either way
    auto free_dir = std::unique_ptr<struct dev_inq_hdr_t, void (*)(void *)>(dir, std::free);

    // Select device to flash
    struct dev_sel_t sel = {0};
    sel.cmd = static_cast<uint8_t>(H8SXCmd::DEVICE_SELECT);
    sel.size = 4;
    for (int i = 0; i < 4; i++)
        sel.code[i] = dir->code[i];
    sel.sum = Checksum((uint8_t *)&sel, sizeof(sel) - 1);
    err = libusb_bulk_transfer(device, BULK_EP_OUT, (uint8_t *)&sel, sizeof(sel), &transferred, 0);
    CHECK_ERR("error in device selection!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error in device selection!");
    if (received < 1 || buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error in device selection!");

    // 0x21 -> Clock Mode Inquiry
    uint8_t cmd = static_cast<uint8_t>(H8SXCmd::CLOCK_MODE_INQUIRY);
    err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
    CHECK_ERR("error during clock mode inquiry!");
    err = libusb_bulk_transfer(device, BULK_EP_IN, (uint8_t *)&buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during clock mode inquiry!");

    // Checksum
    err = libusb_bulk_transfer(device, BULK_EP_IN, &sum, 1, &received, 0);

    // 0x11 -> Clock Mode Selection
    uint8_t csel[] = {0x11, 0x01, 0x01, 0xed};
    err = libusb_bulk_transfer(device, BULK_EP_OUT, (uint8_t *)&csel, sizeof(csel), &transferred, 0);
    CHECK_ERR("error during clock mode selection!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error in clock mode selection!");
    if (received < 1 || buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error in clock mode selection!");

    // 0x27 -> Programming Unit Inquiry
    cmd = static_cast<uint8_t>(H8SXCmd::PROG_UNIT_INQUIRY);
    err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
    CHECK_ERR("error during programming mode inquiry!");
    err = libusb_bulk_transfer(device, BULK_EP_IN, (uint8_t *)&buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during programming mode inquiry!");

    // Checksum
    err = libusb_bulk_transfer(device, BULK_EP_IN, &sum, 1, &received, 0);

    // 0x3F -> New Bit-Rate Selection
    uint8_t bsel[] = {0x3f, 0x07, 0x04, 0x80, 0x06, 0x40,
                      0x02, 0x01, 0x01, 0xec};
    err = libusb_bulk_transfer(device, BULK_EP_OUT, (uint8_t *)&bsel, sizeof(bsel), &transferred, 0);
    CHECK_ERR("error during bit rate selection!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during bit rate selection!");
    if (received < 1 || buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during bit rate selection!");

    // Bit rate confirmation 0x06 ->
    cmd = 0x06;
    err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
    CHECK_ERR("error during bit rate confirmation!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during bit rate confirmation!");
    if (received < 1 || buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during bit rate confirmation!");

    // Transition to Programming/Erasing State 0x40 ->
    cmd = static_cast<uint8_t>(H8SXCmd::BEGIN_PROGRAMMING);
    err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
    CHECK_ERR("error during transition to programming state!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during transition to programming state!");
    if (received < 1 || buf[0] != 0x06)
        err = -1;
    CHECK_ERR("error during transition to programming state!");

    // User MAT Programming Selection 0x43 ->
    cmd = static_cast<uint8_t>(H8SXCmd::USER_MAT_SELECT);
    err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
    CHECK_ERR("error during user MAT programming selection!");

    // Expected response 0x06 <- (ACK)
    err = libusb_bulk_transfer(device, BULK_EP_IN, buf, sizeof(buf), &received, 0);
    CHECK_ERR("error during user MAT programming selection!");
    if (received < 1 || buf[0] != 0x06)
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

auto H8SX::InquireDevice(struct dev_inq_hdr_t **hdr) const -> void
{
    int err = 0;
    int transferred = 0, received = 0;

    //the buffer is handed to the caller on success, and must not leak on any
    //of the error paths CHECK_ERR throws from
    auto buf = (uint8_t *)calloc(1, BUF_SIZE);
    if (buf == nullptr)
    {
        throw std::runtime_error("Out of memory");
    }

    try
    {
        // First command     0x55 -> Begin inquiry phase
        uint8_t cmd = static_cast<uint8_t>(H8SXCmd::BEGIN_INQUIRY);
        err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
        CHECK_ERR("cannot begin inquiry phase!");

        // Expected response 0xE6 <- (ACK)
        err = libusb_bulk_transfer(device, BULK_EP_IN, buf, BUF_SIZE, &received, 0);
        CHECK_ERR("failed to receive reply to inquiry!");
        if (received < 1 || buf[0] != 0xE6)
            err = -1;
        CHECK_ERR("wrong response from radio!");

        // Second command     0x20 -> Supported Device Inquiry
        cmd = static_cast<uint8_t>(H8SXCmd::DEVICE_INQUIRY);
        err = libusb_bulk_transfer(device, BULK_EP_OUT, &cmd, 1, &transferred, 0);
        CHECK_ERR("failed to query supported device!");

        // Expected response  <- Supported Device Response
        err = libusb_bulk_transfer(device, BULK_EP_IN, buf, BUF_SIZE, &received, 0);
        CHECK_ERR("failed to receive supported device response!");
        if ((size_t)received < sizeof(struct dev_inq_hdr_t))
            err = -1;
        CHECK_ERR("short supported device response!");

        auto dir = (struct dev_inq_hdr_t *)buf;

        //nchar comes from the device, it must not be trusted to index the buffer
        if ((size_t)received < sizeof(struct dev_inq_hdr_t) + dir->nchar)
            err = -1;
        CHECK_ERR("device name is longer than the response!");

        //Checksum, read past the response rather than into buf[0], which used
        //to overwrite the first byte of the header we just received
        int sum_received = 0;
        uint8_t sum = 0;
        err = libusb_bulk_transfer(device, BULK_EP_IN, &sum, 1, &sum_received, 0);
        CHECK_ERR("failed to receive checksum!");

        // TODO: Validate checksum
        *hdr = dir;
    }
    catch (...)
    {
        free(buf);
        throw;
    }
}
