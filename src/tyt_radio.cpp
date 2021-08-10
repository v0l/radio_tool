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
#include <radio_tool/radio/tyt_radio.hpp>
#include <radio_tool/fw/tyt_fw.hpp>
#include <radio_tool/util/flash.hpp>

#include <math.h>
#include <iomanip>
#include <iostream>
#include <vector>

using namespace radio_tool::radio;

auto TYTRadio::ToString() const -> const std::string
{
    std::stringstream out;

    auto model = dfu.IdentifyDevice();
    auto time = dfu.GetTime();

    out << "== TYT Radio Info ==" << std::endl
        << "Radio: " << model << std::endl
        << "RTC: " << ctime(&time);

    return out.str();
}

auto TYTRadio::WriteFirmware(const std::string &file) const -> void
{
    constexpr auto TransferSize = 1024u;

    auto fw = fw::TYTFW();
    fw.Read(file);

    dfu.SendTYTCommand(dfu::TYTCommand::FirmwareUpgrade);
    for (auto &r : fw.GetDataSegments())
    {
        flash::FlashUtil::AlignedContiguousMemoryOp(flash::STM32F40X, r.address, r.address + r.size, [this](const uint32_t &addr, const uint32_t &size, const flash::FlashSector &sector) {
            std::cerr << "Erasing: 0x" << std::setw(8) << std::setfill('0') << std::hex << addr
                      << " [Size=0x" << std::hex << size << "]" << std::endl
                      << "-- " << sector.ToString() << std::endl;

            dfu.Erase(addr);
        });

        auto b_offset = 0u;
        flash::FlashUtil::AlignedContiguousMemoryOp(flash::STM32F40X, r.address, r.address + r.size, [this, &fw, &r, &TransferSize, &b_offset](const uint32_t &addr, const uint32_t &size, const flash::FlashSector &sector) {
            const auto& binary_data = r.data;
            const auto blocks = (int)std::ceil(size / (double)TransferSize);

            std::cerr << "Writing: 0x" << std::setw(8) << std::setfill('0') << std::hex << addr
                      << " [Size=0x" << std::hex << size << "]" << std::endl;
            dfu.SetAddress(addr);
            for(auto wValue = 0; wValue < blocks; wValue++) 
            {
                auto block_offset = b_offset + (TransferSize * wValue);
                auto to_write = std::vector<uint8_t>(
                    binary_data.begin() + block_offset, 
                    binary_data.begin() + block_offset + std::min(TransferSize, r.size - block_offset)
                );

                /*std::cerr
                    << "-- wValue=0x" << std::setw(2) << std::setfill('0') << std::hex << (2 + wValue)
                    << ", Size=0x" << to_write.size()
                    << std::endl;*/
                dfu.Download(to_write, 2 + wValue);
            }
            b_offset += size;
        });
    }
}