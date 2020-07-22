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
#pragma once

#include <radio_tool/radio/radio.hpp>
#include <radio_tool/dfu/tyt_dfu.hpp>

namespace radio_tool::radio
{
    static constexpr auto BlockSize = 2048;

    class TYTRadio : public RadioSupport
    {
    public:
        TYTRadio(libusb_device_handle* h)
            : dfu(h) {}

        auto WriteFirmware(const std::string &file) const -> void override;
        auto ToString() const -> const std::string override;

        static auto SupportsDevice(const libusb_device_descriptor &dev) -> bool
        {
            if (dev.idVendor == dfu::TYTDFU::VID && dev.idProduct == dfu::TYTDFU::PID)
            {
                return true;
            }
            return false;
        }

        /**
         * Get the handler used to communicate with this device
         */
        auto GetDFU() const -> const dfu::TYTDFU& override
        {
            return dfu;
        }

        static auto Create(libusb_device_handle* h) -> std::unique_ptr<TYTRadio> {
            return std::unique_ptr<TYTRadio>(new TYTRadio(h));
        }
    private:
        uint16_t dev_index;
        const dfu::TYTDFU dfu;
    };
} // namespace radio_tool::radio