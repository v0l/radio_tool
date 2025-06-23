#pragma once

#include <string>
#include <vector>
#include <fstream>
#include <iostream>
#include <iomanip>

#include <stdint.h>
#include <memory.h>

namespace radio_tool
{
    class XORTool
    {
    public:
        /**
         * Guess XOR key by assuming 0x00 is the most common byte value
         * For any position in the key we would expect to see (0x00 ^ key[n]) the most times
         * We also assume the key is 1024 bytes long
         */
        static auto MakeXOR(const std::vector<uint8_t> &in_binary) -> std::vector<uint8_t>
        {
            constexpr auto KeyLen = 1024;

            uint16_t matrix[KeyLen][256];
            memset(matrix, 0, sizeof(uint16_t) * KeyLen * 256);

            auto i_x = 0;
            for (const auto &b : in_binary)
            {
                auto i = i_x++ % KeyLen;
                matrix[i][b]++;
            }

            uint8_t key[KeyLen];
            memset(key, 0, KeyLen);

            for (int i = 0; i < KeyLen; i++)
            {
                uint8_t highest = 0;
                uint8_t highestval = 0;
                for (int b = 0; b < 256; b++)
                {
                    uint8_t t = matrix[i][b];
                    if (highest < t)
                    {
                        highest = t;
                        highestval = b;
                    }
                }
                key[i] = highestval;
            }

            return std::vector<uint8_t>(key, key + KeyLen);
        }

        /**
         * Verify the vector table and stack top are valid addresses
         */
        static auto Verify(const uint32_t &base_address, const std::vector<uint8_t> &in_binary, const std::vector<uint8_t> &key) -> bool
        {
            auto address_max = base_address + in_binary.size();
            constexpr auto table_size = 0x61u; // 0x60 + 1 for stack top
            uint32_t vector_table[table_size];

            // copy data from in binary to our vector_table and apply xor
            std::copy(in_binary.begin(), in_binary.begin() + (table_size * sizeof(uint32_t)), (uint8_t *)vector_table);
            for (auto x = 0; x < table_size * sizeof(uint32_t); x++)
            {
                ((uint8_t *)vector_table)[x] = ((uint8_t *)vector_table)[x] ^ key[x % key.size()];
            }

            // check stack_top
            if ((vector_table[0] & 0x2FFE0000u) != 0x20000000u)
            {
                std::cerr << "Stack top invalid" << std::endl;
                return false;
            }

            // check interupts
            for (auto irh = 1u; irh < table_size; irh++)
            {
                auto irq_addr = vector_table[irh];
                if (irq_addr != 0 && (irq_addr <= base_address || irq_addr >= address_max))
                {
                    std::cerr
                        << "Invalid vector_table address: [" << irh << "]-0x"
                        << std::setfill('0') << std::setw(8) << std::hex << irq_addr
                        << std::endl
                        << "  Input: 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)in_binary[(irh * 4)] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)in_binary[(irh * 4) + 1] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)in_binary[(irh * 4) + 2] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)in_binary[(irh * 4) + 3] << std::endl
                        << "  Key:   0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)key[(irh * 4)] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)key[(irh * 4) + 1] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)key[(irh * 4) + 2] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)key[(irh * 4) + 3] << std::endl
                        << "  Out:   0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)((uint8_t *)vector_table)[(irh * 4)] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)((uint8_t *)vector_table)[(irh * 4) + 1] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)((uint8_t *)vector_table)[(irh * 4) + 2] << " 0x"
                        << std::setfill('0') << std::setw(2) << std::hex << (int)((uint8_t *)vector_table)[(irh * 4) + 3] << std::endl;

                    return false;
                }
            }

            return true;
        }
    };
} // namespace radio_tool