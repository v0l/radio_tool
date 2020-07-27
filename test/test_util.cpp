#include <radio_tool/util.hpp>

#include <assert.h>

using namespace radio_tool;
int main(int argc, char **argv)
{
    std::vector<uint8_t> t1 = {'a', 'b', 'c', 'd', 'e'};
    std::vector<uint8_t> t2 = {'a', 'b', 'c', 'd', 'e', 'f'};
    std::vector<uint8_t> t3 = {'a', 'b', 'c', 'd', 'e', 'f', 'g'};

    auto t1_i = t1.begin();
    auto t2_i = t2.begin();
    auto t3_i = t3.begin();

    assert(Fletcher16(t1_i, t1.size()) == 0xC8F0);
    assert(Fletcher16(t2_i, t2.size()) == 0x2057);
    assert(Fletcher16(t3_i, t3.size()) == 0x0627);
}