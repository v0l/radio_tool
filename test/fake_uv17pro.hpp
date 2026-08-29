/**
 * This file is part of radio_tool.
 * Copyright (c) 2026 v0l <radio_tool@v0l.io>
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

#include <radio_tool/device/byte_stream.hpp>
#include <radio_tool/radio/uv17pro_radio.hpp>

#include <deque>
#include <map>
#include <stdexcept>
#include <string>
#include <vector>

namespace radio_tool::test
{
    /**
     * A Baofeng UV-17Pro speaking its clone protocol over an in memory byte
     * stream, so the driver can be tested without a radio.
     *
     * The radio only answers what it is asked for, and it records anything the
     * driver does wrong (a write outside a declared memory region, a block
     * which is not a whole number of bytes inside a region) rather than
     * quietly accepting it.
     */
    class FakeUV17Pro : public device::ByteStream
    {
    public:
        FakeUV17Pro(const radio_tool::radio::UV17ProRadio::UV17ProModel &model, const size_t &block_hint = 0x40)
            : model(model), block_hint(block_hint)
        {
            //fill every region with a recognisable pattern
            uint8_t v = 0;
            for (const auto &r : model.regions)
            {
                for (auto a = 0u; a < r.size; a++)
                {
                    memory[(uint16_t)(r.start + a)] = v++;
                }
            }
        }

        auto Write(const std::vector<uint8_t> &data) const -> void override
        {
            rx.insert(rx.end(), data.begin(), data.end());
            Process();
        }

        auto Read(const size_t &len, const uint32_t &) const -> std::vector<uint8_t> override
        {
            auto take = std::min(len, tx.size());
            std::vector<uint8_t> out(tx.begin(), tx.begin() + take);
            tx.erase(tx.begin(), tx.begin() + take);
            return out;
        }

        auto FlushInput() const -> void override
        {
            tx.clear();
        }

        auto BlockSizeHint() const -> size_t override
        {
            return block_hint;
        }

        /**
         * The memory as the radio holds it, after any writes
         */
        auto Memory() const -> const std::map<uint16_t, uint8_t> &
        {
            return memory;
        }

        /**
         * Anything the driver did which a real radio would not have liked
         */
        auto Complaints() const -> const std::vector<std::string> &
        {
            return complaints;
        }

        auto BlocksWritten() const -> size_t
        {
            return blocks_written;
        }

    private:
        auto Complain(const std::string &what) const -> void
        {
            complaints.push_back(what);
        }

        auto InRegion(const uint32_t &addr, const uint32_t &len) const -> bool
        {
            for (const auto &r : model.regions)
            {
                if (addr >= r.start && (addr + len) <= (uint32_t)(r.start + r.size))
                {
                    return true;
                }
            }
            return false;
        }

        auto Send(const std::vector<uint8_t> &data) const -> void
        {
            tx.insert(tx.end(), data.begin(), data.end());
        }

        auto Take(const size_t &n) const -> std::vector<uint8_t>
        {
            std::vector<uint8_t> ret(rx.begin(), rx.begin() + n);
            rx.erase(rx.begin(), rx.begin() + n);
            return ret;
        }

        /**
         * Consume as many whole commands as the buffer holds
         */
        auto Process() const -> void
        {
            auto ident = std::string(model.ident);

            while (!rx.empty())
            {
                if (!identified)
                {
                    if (rx.size() < ident.size())
                    {
                        return;
                    }
                    auto got = std::string(rx.begin(), rx.begin() + ident.size());
                    if (got != ident)
                    {
                        Complain("wrong ident magic: " + got);
                        rx.clear();
                        return;
                    }
                    Take(ident.size());
                    identified = true;
                    Send({0x06});
                    continue;
                }

                if (handshake < 3)
                {
                    //0x46 -> 16 bytes, 0x4d -> 15 bytes, SEND!... -> 1 byte
                    if (rx.front() == 0x46)
                    {
                        Take(1);
                        auto info = std::string("UV17PROFAKE-0001");
                        Send(std::vector<uint8_t>(info.begin(), info.end()));
                        handshake++;
                        continue;
                    }
                    if (rx.front() == 0x4d)
                    {
                        Take(1);
                        Send(std::vector<uint8_t>(15, 0x30));
                        handshake++;
                        continue;
                    }
                    if (rx.front() == 0x53)
                    {
                        if (rx.size() < 25)
                        {
                            return;
                        }
                        Take(25);
                        Send({0x06});
                        handshake++;
                        continue;
                    }
                }

                auto cmd = rx.front();
                if (cmd == 'R')
                {
                    if (rx.size() < 4)
                    {
                        return;
                    }
                    auto hdr = Take(4);
                    auto addr = (uint32_t)((hdr[1] << 8) | hdr[2]);
                    auto size = (uint32_t)hdr[3];

                    if (!InRegion(addr, size))
                    {
                        Complain("read outside a memory region");
                    }

                    std::vector<uint8_t> block;
                    for (auto ix = 0u; ix < size; ix++)
                    {
                        block.push_back(memory[(uint16_t)(addr + ix)]);
                    }

                    Send({'R', hdr[1], hdr[2], hdr[3]});
                    Send(radio_tool::radio::UV17ProRadio::Crypt(model.encryption_key, block));
                    continue;
                }

                if (cmd == 'W')
                {
                    if (rx.size() < 4)
                    {
                        return;
                    }
                    auto size = (size_t)rx[3];
                    if (rx.size() < 4 + size)
                    {
                        return;
                    }
                    auto hdr = Take(4);
                    auto payload = Take(size);
                    auto addr = (uint32_t)((hdr[1] << 8) | hdr[2]);

                    if (size == 0)
                    {
                        Complain("empty write block");
                    }
                    if (!InRegion(addr, (uint32_t)size))
                    {
                        Complain("write outside a memory region at 0x" + std::to_string(addr) +
                                 " for " + std::to_string(size) + " bytes");
                    }

                    auto plain = radio_tool::radio::UV17ProRadio::Crypt(model.encryption_key, payload);
                    for (size_t ix = 0; ix < plain.size(); ix++)
                    {
                        memory[(uint16_t)(addr + ix)] = plain[ix];
                    }

                    blocks_written++;
                    Send({0x06});
                    continue;
                }

                Complain("unknown command byte");
                rx.clear();
                return;
            }
        }

        const radio_tool::radio::UV17ProRadio::UV17ProModel model;
        const size_t block_hint;

        mutable std::map<uint16_t, uint8_t> memory;
        mutable std::vector<uint8_t> rx, tx;
        mutable std::vector<std::string> complaints;
        mutable bool identified = false;
        mutable int handshake = 0;
        mutable size_t blocks_written = 0;
    };
} // namespace radio_tool::test
