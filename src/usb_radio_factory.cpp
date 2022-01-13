/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
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

#include <radio_tool/radio/usb_radio_factory.hpp>
#include <radio_tool/device/device.hpp>
#include <radio_tool/radio/tyt_radio.hpp>

#include <libusb-1.0/libusb.h>

#include <exception>
#include <functional>
#include <codecvt>
#include <cstring>
#include <iostream>

using namespace radio_tool::radio;

struct DeviceMapper
{
    std::function<bool(const libusb_device_descriptor &)> SupportsDevice;
    std::function<const RadioOperations *(libusb_device_handle *)> CreateOperations;
};

/**
 * A list of functions to test each radio handler
 */
const std::vector<DeviceMapper> RadioSupports = {
    {TYTRadio::SupportsDevice, TYTRadio::Create},
};

USBRadioFactory::USBRadioFactory() : usb_ctx(nullptr)
{
    usb_ctx = CreateContext();
}

USBRadioFactory::~USBRadioFactory()
{
    libusb_exit(usb_ctx);
    usb_ctx = nullptr;
}

auto USBRadioFactory::ListDevices(const uint16_t &idx_offset) const -> const std::vector<RadioInfo *>
{
    std::vector<RadioInfo *> ret;

    libusb_device **devs;
    auto ndev = libusb_get_device_list(usb_ctx, &devs);
    int err = LIBUSB_SUCCESS;
    auto n_idx = 0;

    if (ndev >= 0)
    {
        for (auto x = 0; x < ndev; x++)
        {
            libusb_device_descriptor desc;
            if (LIBUSB_SUCCESS == (err = libusb_get_device_descriptor(devs[x], &desc)))
            {
                for (const auto &fnSupport : RadioSupports)
                {
                    if (fnSupport.SupportsDevice(desc))
                    {
                        int err = LIBUSB_SUCCESS;
                        libusb_device_handle *h;
                        auto cdev = const_cast<libusb_device *>(devs[x]);
                        if (LIBUSB_SUCCESS == (err = libusb_open(cdev, &h)))
                        {
                            auto mfg = GetDeviceString(desc.iManufacturer, h),
                                 prd = GetDeviceString(desc.iProduct, h);

                            auto bus = libusb_get_bus_number(cdev);
                            auto port = libusb_get_port_number(cdev);

                            auto fnOpen = [bus, port, &fnSupport]()
                            {
                                auto openDev = OpenDevice(bus, port);
                                return fnSupport.CreateOperations(openDev);
                            };

                            auto nInf = new USBRadioInfo(fnOpen, mfg, prd, desc.idVendor, desc.idProduct, idx_offset + n_idx);
                            ret.push_back(nInf);
                            n_idx++;
                            libusb_close(h);
                        }
                        else
                        {
                            std::cerr << "Failed to open device VID=0x"
                                      << std::hex << std::setw(4) << std::setfill('0') << desc.idVendor
                                      << ", PID=0x"
                                      << std::hex << std::setw(4) << std::setfill('0') << desc.idProduct
                                      << " (" << libusb_error_name(err) << ")"
                                      << std::endl;
                        }
                    }
                }
            }
        }

        libusb_free_device_list(devs, 1);
    }
    else
    {
        throw std::runtime_error(libusb_error_name(ndev));
    }
    return ret;
}

auto USBRadioFactory::GetDeviceString(const uint8_t &desc, libusb_device_handle *h) const -> std::wstring
{
    auto err = 0;
    size_t prd_len = 0;
    unsigned char lang[42], prd[255];
    memset(prd, 0, 255);

    libusb_get_string_descriptor(h, 0, 0, lang, 42);
    if (0 > (prd_len = libusb_get_string_descriptor(h, desc, lang[2] << 8 | lang[3], prd, 255)))
    {
        throw std::runtime_error(libusb_error_name(err));
    }

    //Encoded as UTF-16 (LE), Prefixed with length and some other byte.
    typedef std::codecvt_utf16<char16_t, 1114111UL, std::little_endian> cvt;
    auto u16 = std::wstring_convert<cvt, char16_t>().from_bytes((const char *)prd + 2, (const char *)prd + prd_len);
    return std::wstring(u16.begin(), u16.end());
}

auto USBRadioFactory::OpenDevice(const uint8_t &bus, const uint8_t &port) -> libusb_device_handle *
{
    auto usb_ctx = CreateContext();

    libusb_device **devs;
    auto ndev = libusb_get_device_list(usb_ctx, &devs);
    int err = LIBUSB_SUCCESS;
    auto n_idx = 0;

    if (ndev >= 0)
    {
        for (auto x = 0; x < ndev; x++)
        {
            auto b = libusb_get_bus_number(devs[x]);
            auto p = libusb_get_port_number(devs[x]);
            if (b == bus && p == port)
            {
                libusb_device_handle *handle;

                if (libusb_open(devs[x], &handle) == LIBUSB_SUCCESS)
                {
                    return handle;
                }
                break;
            }
        }

        libusb_free_device_list(devs, 1);
    }
    else
    {
        throw std::runtime_error(libusb_error_name(ndev));
    }

    return nullptr;
}

auto USBRadioFactory::CreateContext() -> libusb_context *
{
    libusb_context *usb_ctx;

    auto err = libusb_init(&usb_ctx);
    if (err != LIBUSB_SUCCESS)
    {
        throw std::runtime_error(libusb_error_name(err));
    }
#if defined(LIBUSB_API_VERSION) && (LIBUSB_API_VERSION >= 0x01000107)
    libusb_set_log_cb(
        usb_ctx, [](libusb_context *, enum libusb_log_level, const char *str)
        { std::wcout << str << std::endl; },
        LIBUSB_LOG_CB_CONTEXT);
#endif

    return usb_ctx;
}