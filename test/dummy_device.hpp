#pragma once

#include <radio_tool/dfu/dfu.hpp>
#include <radio_tool/util/flash.hpp>

#include <string>
#include <fstream>

using namespace radio_tool::dfu;
using namespace radio_tool::flash;

namespace radio_tool::test
{
    /**
     * A dummy device to check firwmare writing process is correct
     */
    class DummyDevice : DFU
    {
    public:
        DummyDevice(const std::string& file, const FlashMap &map) 
            : DFU(nullptr), file(file), map(map), address(0), state(DFUState::DFU_IDLE), status(DFUStatus::OK)
        {
            out = std::ofstream(file, std::ios_base::out | std::ios_base::binary);
        }

        auto SetAddress(const uint32_t& addr) -> void
        {
            address = addr;
            SeekToAddr(address);
        }

        auto Erase(const uint32_t&) const -> void 
        {
            //nothing
        }

        auto Download(const std::vector<uint8_t> &data, const uint16_t &wValue = 0) -> void
        {
            auto addr_offset = address + (data.size() * (wValue - 2));

            SeekToAddr(addr_offset);
            out.write((const char*)data.data(), data.size());
            state = DFUState::DFU_DOWNLOAD_BUSY;
        }

        auto Upload(const uint16_t &size, const uint8_t &wValue = 0) -> std::vector<uint8_t> 
        {
            state = DFUState::DFU_UPLOAD_BUSY;
            return std::vector<uint8_t>();
        }

        auto GetState() const -> DFUState 
        {
            return state;
        }
        auto GetStatus() -> const DFUStatusReport
        {
            auto ret = DFUStatusReport(status, 0, state, 0);
            if(state == DFUState::DFU_DOWNLOAD_BUSY) {
                state = DFUState::DFU_DOWNLOAD_IDLE;
            }
            if(state == DFUState::DFU_UPLOAD_BUSY)
            {
                state = DFUState::DFU_UPLOAD_IDLE;
            }
            return ret;
        }
        auto Abort() -> void
        {
            state = DFUState::DFU_IDLE;
        }
        auto Detach() const -> void
        {
            //nothing
        }
    private:
        const std::string file;
        const FlashMap map;
        std::ofstream out;

        uint32_t address;
        DFUState state;
        DFUStatus status;

        constexpr auto FilePos(const uint32_t &addr) const -> const uint32_t
        {
            return address - map.front().start;
        }

        constexpr auto SeekToAddr(const uint32_t &addr) -> void 
        {
            out.seekp(FilePos(addr));
        }
    };
} // namespace radio_tool::test