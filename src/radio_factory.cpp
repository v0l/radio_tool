/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 Kieran Harkin <kieran+git@harkin.me>
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
#include <radio_tool/radio/radio_factory.hpp>

#include <radio_tool/radio/tyt_radio.hpp>

#include <libusb-1.0/libusb.h>

#include <exception>
#include <functional>
#include <codecvt>
#include <cstring>

using namespace radio_tool::radio;

/**
 * A list of functions to test each radio handler
 */
const std::vector<std::pair<std::function<bool(const libusb_device_descriptor &)>, std::function<std::unique_ptr<RadioSupport>(libusb_device_handle *)>>> RadioSupports = {
    {TYTRadio::SupportsDevice, TYTRadio::Create}};

auto RadioFactory::GetRadioSupport(const uint16_t &dev_idx) const -> std::unique_ptr<RadioSupport>
{
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
                    if (fnSupport.first(desc))
                    {
                        if (n_idx == dev_idx)
                        {
                            libusb_device_handle *h;
                            if (LIBUSB_SUCCESS == (err = libusb_open(devs[x], &h)))
                            {
                                libusb_free_device_list(devs, 1);
                                return fnSupport.second(h);
                            }
                            else
                            {
                                libusb_free_device_list(devs, 1);
                                throw std::runtime_error("Failed to open device");
                            }
                        }
                        n_idx++;
                        break;
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
    throw std::runtime_error("Radio not supported");
}

auto RadioFactory::OpDeviceList(std::function<void(const libusb_device *, const libusb_device_descriptor &, const uint16_t &)> op) const -> void
{
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
                    if (fnSupport.first(desc))
                    {
                        op(devs[x], desc, n_idx);
                        n_idx++;
                        break;
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
}

auto RadioFactory::ListDevices() const -> const std::vector<RadioInfo>
{
    std::vector<RadioInfo> ret;

    OpDeviceList([&ret, this](const libusb_device *dev, const libusb_device_descriptor &desc, const uint16_t &idx) {
        int err = LIBUSB_SUCCESS;
        libusb_device_handle *h;
        if (LIBUSB_SUCCESS == (err = libusb_open(const_cast<libusb_device *>(dev), &h)))
        {
            auto mfg = GetDeviceString(desc.iManufacturer, h),
                 prd = GetDeviceString(desc.iProduct, h);

            auto nInf = RadioInfo(mfg, prd, desc.idVendor, desc.idProduct, idx);
            ret.push_back(nInf);
            libusb_close(h);
        }
    });

    return ret;
}

auto RadioFactory::GetDeviceString(const uint8_t &desc, libusb_device_handle *h) const -> std::wstring
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

    return std::wstring_convert<std::codecvt_utf8<wchar_t>>().from_bytes((const char *)prd + 1, (const char *)prd + prd_len);
}