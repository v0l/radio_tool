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

#include <radio_tool/fw/fw.hpp>
#include <radio_tool/fw/cipher/uv3x0.hpp>
#include <radio_tool/fw/cipher/dm1701.hpp>
#include <radio_tool/fw/cipher/md380.hpp>
#include <radio_tool/fw/cipher/md9600.hpp>

#include <fstream>
#include <cstring>
#include <sstream>
#include <iomanip>

namespace radio_tool::fw
{
    namespace tyt::magic
    {
        using namespace std::literals::string_literals;

        //OutSecurityBin\0\0
        const std::vector<uint8_t> begin = {0x4f, 0x75, 0x74, 0x53, 0x65, 0x63, 0x75, 0x72, 0x69, 0x74, 0x79, 0x42, 0x69, 0x6e, 0x00, 0x00};
        //OutSecurityBinEnd
        const std::vector<uint8_t> end = {0x4f, 0x75, 0x74, 0x70, 0x75, 0x74, 0x42, 0x69, 0x6e, 0x44, 0x61, 0x74, 0x61, 0x45, 0x6e, 0x64};

        /*
         * +GPS = Both GPS and Non-GPS versions have the same magic value
         * CSV = DMR Database upload as CSV support
         * REC = Recording
         */
        const std::vector<uint8_t> MD2017_D = {0x02, 0x19, 0x0c}; //MD-2017 (REC)
        const std::vector<uint8_t> MD2017_S = {0x02, 0x18, 0x0c}; //MD-2017 GPS (REC)
        const std::vector<uint8_t> MD2017_V = {0x01, 0x19};       //MD-2017 (CSV)
        const std::vector<uint8_t> MD2017_P = {0x01, 0x18};       //MD-2017 GPS (CSV)

        const std::vector<uint8_t> MD9600 = {0x01, 0x14}; //MD-9600 (REC/CSV) +GPS

        const std::vector<uint8_t> UV3X0_GPS = {0x02, 0x16, 0x0c}; //MD-UV3X0 (REC/CSV)(GPS) / RT3S
        const std::vector<uint8_t> UV3X0 = {0x02, 0x17, 0x0c};     //MD-UV3X0 (REC/CSV) / RT3S

        const std::vector<uint8_t> DM1701 = {0x01, 0x0f}; //DM-1701

        const std::vector<uint8_t> MD390 = {0x01, 0x10}; //MD-390
        const std::vector<uint8_t> MD380 = {0x01, 0x0d}; //MD-380 / MD-446
        const std::vector<uint8_t> MD280 = {0x01, 0x1b}; //MD-280

        const std::vector<std::pair<const std::string, const std::vector<uint8_t>>> All = {
            {"MD2017"s, MD2017_D},
            {"MD2017 GPS"s, MD2017_S},
            {"MD2017"s, MD2017_V},
            {"MD2017 GPS"s, MD2017_P},
            {"MD9600"s, MD9600},
            {"UV3X0 GPS"s, UV3X0_GPS},
            {"UV3X0"s, UV3X0},
            {"DM1701"s, DM1701},
            {"MD390"s, MD390},
            {"MD380"s, MD380},
            {"MD280"s, MD280}
        };
    } // namespace tyt::magic

    namespace tyt::cipher
    {
        using namespace std::literals::string_literals;
        using namespace fw::cipher;

        const std::vector<std::tuple<const std::string, const uint8_t *, const uint16_t>> All = {
            {"MD2017"s, uv3x0, uv3x0_length},
            {"MD2017 GPS"s, uv3x0, uv3x0_length},
            {"MD9600"s, md9600, md9600_length},
            {"UV3X0"s, uv3x0, uv3x0_length},
            {"UV3X0 GPS"s, uv3x0, uv3x0_length},
            {"DM1701"s, dm1701, dm1701_length},
            {"MD390"s, md380, md380_length},
            {"MD380"s, md380, md380_length},
            {"MD280"s, md380, md380_length}
        };
    }

    /**
     * Stores the start of the TYT Firmware file header
     */
    typedef struct
    {
        uint8_t magic[16];
        uint8_t radio[16];
        uint32_t n1, n2, n3, n4;
        uint8_t counter_magic[76];
        uint32_t n_regions;
    } TYTFirmwareHeader;
    static_assert(sizeof(TYTFirmwareHeader) == 128);

    class TYTFW : public FirmwareSupport
    {
    public:
        TYTFW() {}
        TYTFW(const std::vector<uint8_t> &cMagic)
            : counterMagic(cMagic)
        {
        }

        auto Read(const std::string &file) -> void override;
        auto Write(const std::string &file) -> void override;
        auto ToString() const -> std::string override;
        auto Decrypt() -> void override;
        auto Encrypt() -> void override;

        /**
         * @note This is not the "radio group" which exists in the firmware header
         */
        auto GetRadioModel() const -> const std::string override;

        /**
         * Get the counter magic for a specific model radio
         * @note This is the radio model not the model from the firmware file
         */
        static auto GetCounterMagic(const std::string &radio) -> const std::vector<uint8_t>
        {
            for (const auto &r : tyt::magic::All)
            {
                if (r.first.compare(radio) == 0)
                {
                    return r.second;
                }
            }
            throw std::runtime_error("Radio not supported");
        }

        /**
         * Return the radio model of a specific counter magic sequence
         * @note This is probably not accurate unless you already know its a TYT firmware file
         */
        static auto GetRadioFromMagic(const std::vector<uint8_t> &cm) -> const std::string
        {
            for (const auto &r : tyt::magic::All)
            {
                if (r.second.size() != cm.size())
                    continue;
                /* GCC doesnt seem to mind which is longer, MSVC tries to read past the end of [first2] */
                if (std::equal(cm.begin(), cm.end(), r.second.begin()))
                {
                    return r.first;
                }
            }
            throw std::runtime_error("Radio not supported");
        }

        static auto SupportsFirmwareFile(const std::string &file) -> bool;
        static auto Create() -> std::unique_ptr<FirmwareSupport>
        {
            return std::make_unique<TYTFW>();
        }

    private:
        std::vector<uint8_t> counterMagic; //2-3 bytes

        uint32_t n1,
            n2, // appears to be some kind of bootloader version
            n3, n4;
        std::string radio;

        static auto ReadHeader(std::ifstream &) -> TYTFirmwareHeader;
        static auto CheckHeader(const TYTFirmwareHeader &) -> void;
        auto ApplyXOR() -> void;
    };

} // namespace radio_tool::fw