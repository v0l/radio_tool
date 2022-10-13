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
#pragma once

#include <stdint.h>
#include <vector>
#include <string>
#include <sstream>
#include <iomanip>
#include <iostream>

#include <libusb-1.0/libusb.h>
#include <radio_tool/h8sx/h8sx_exception.hpp>

#define CHECK_ERR(errstr)                                               \
    do {                                                                \
        if (err < LIBUSB_SUCCESS) {                                     \
            std::string e = std::string(errstr) +                       \
                            std::string(libusb_error_name(err));        \
            throw radio_tool::h8sx::H8SXException(e);                   \
        }                                                               \
    } while(0)
#define PACK __attribute__((__packed__))

#define BULK_EP_IN   0x82
#define BULK_EP_OUT  0x01
#define BUF_SIZE 64 * 1024 // Max transfer size 64KB

namespace radio_tool::h8sx
{
    enum class H8SXCmd : uint8_t
    {
        /*
         * Begin the inquiry phase
         */
        BEGIN_INQUIRY = 0x55,

        /*
         * Inquiry regarding device codes
         */
        DEVICE_INQUIRY = 0x20,

        /*
         * Selection of device code
         */
        DEVICE_SELECT = 0x10,

        /*
         * Inquiry regarding numbers of clock modes
         * and values of each mode
         */
        CLOCK_MODE_INQUIRY = 0x21,

        /*
         * Indication of the selected clock mode
         */
        CLOCK_MODE_SELECT = 0x11,

        /*
         * Inquiry regarding the unit of program data
         */
        PROG_UNIT_INQUIRY = 0x27,

        /*
         * Selection of new bit rate
         */
        BITRATE_SELECT = 0x3F,

        /*
         * Erasing of user MAT and user boot MAT, and
         * entry to programming/erasing state
         */
        BEGIN_PROGRAMMING = 0x40,

        /*
         * Transfers the user MAT programming
         * program
         */
        USER_MAT_SELECT = 0x43,

        /*
         * Programs 128 bytes of data
         */
        PROGRAM_128B = 0x50,

        /*
         * Checks the checksum of the user MAT
         */
        USER_MAT_CHECKSUM = 0x4B,
    };

    // Supported device inquiry response
    struct PACK dev_inq_hdr_t {
        uint8_t cmd = 0;
        uint8_t size = 0;
        uint8_t ndev = 0;
        uint8_t nchar = 0;
        char code[4] = { 0 };
    };

    struct PACK dev_sel_t {
        uint8_t cmd = 0;
        uint8_t size = 0;
        char code[4] = { 0 };
        uint8_t sum = 0;
    };

    struct PACK prog_chunk_t {
        uint8_t cmd = static_cast<uint8_t>(H8SXCmd::PROGRAM_128B);
        uint32_t addr = 0;
        uint8_t data[1024] = { 0 };
        uint8_t sum = 0;
    };

    struct PACK prog_end_t {
        uint8_t cmd = static_cast<uint8_t>(H8SXCmd::PROGRAM_128B);
        uint32_t addr = 0xffffffff;
        uint8_t sum = 0xb4;
    };

    struct PACK sum_chk_t {
        uint8_t cmd;
        uint8_t size;
        uint32_t chk;
        uint8_t sum;
    };

    class H8SX
    {
    public:
        H8SX(libusb_device_handle *device)
            : timeout(5000), device(device) {}

        auto Init() const -> void;
        auto IdentifyDevice() -> std::string;
        auto Download(const std::vector<uint8_t> &) const -> void;

    private:
        libusb_context *usb_ctx;

        auto GetDeviceString(const libusb_device_descriptor &, libusb_device_handle *) const -> std::wstring;
        auto Checksum(const uint8_t *data, const size_t len) const -> uint8_t;

    protected:
        const uint16_t timeout;
        libusb_device_handle *device;
        struct dev_inq_hdr_t *dir;

        auto CheckDevice() const -> void;

        /**
         * Ensures the state is H8SX_IDLE or H8SX_DNLOAD_IDLE
         */
        auto InitDownload() const -> void;

        /**
         * Ensures the state is H8SX_IDLE or H8SX_DPLOAD_IDLE
         */
        auto InitUpload() const -> void;
    };
} // namespace radio_tool::h8sx
