#include <radio_tool/util.hpp>
#include <radio_tool/xor_tool.hpp>

#include <assert.h>
#include <cstring>

using namespace radio_tool;
int main()
{
    std::vector<uint8_t> t1 = {'a', 'b', 'c', 'd', 'e'};
    std::vector<uint8_t> t2 = {'a', 'b', 'c', 'd', 'e', 'f'};
    std::vector<uint8_t> t3 = {'a', 'b', 'c', 'd', 'e', 'f', 'g'};

    auto t1_i = t1.begin();
    auto t2_i = t2.begin();
    auto t3_i = t3.begin();

    assert(Fletcher16(t1_i, t1.size()) == 0xC8F0);
    assert(Fletcher16(t2_i, t2.size()) == 0x2057);
    assert(Fletcher16(t3_i, t3.size()) == 0xDEBE);

    //FormatBytes covers every unit, MiB used to be missing from the chain
    assert(FormatBytes(512) == "512 B");
    assert(FormatBytes(2048) == "2.00 kiB");
    assert(FormatBytes(4ULL * MiB) == "4.00 MiB");
    assert(FormatBytes(3ULL * GiB) == "3.00 GiB");

    //the XOR key histogram must not wrap when a byte occurs more than 255 times
    std::vector<uint8_t> big(1024 * 300, 0x00);
    for (size_t i = 0; i < big.size(); i += 1024)
    {
        big[i] = 0xAA; //0xAA occurs 300 times at key offset 0
    }
    auto key = XORTool::MakeXOR(big);
    assert(key.size() == 1024);
    assert(key[0] == 0xAA);
    assert(key[1] == 0x00);
}