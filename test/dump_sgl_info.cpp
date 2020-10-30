#include <iostream>
#include <fstream>
#include <iomanip>
#include <iterator>
#include <vector>

#define HEX(x) std::setw(x) << std::setfill('0') << std::hex

void HexDump(const void* data, size_t size)
{
    char ascii[17];
    size_t i, j;
    ascii[16] = '\0';
    for (i = 0; i < size; ++i) {
        printf("%02X ", ((unsigned char*)data)[i]);
        if (((unsigned char*)data)[i] >= ' ' && ((unsigned char*)data)[i] <= '~') {
            ascii[i % 16] = ((unsigned char*)data)[i];
        }
        else {
            ascii[i % 16] = '.';
        }
        if ((i + 1) % 8 == 0 || i + 1 == size) {
            printf(" ");
            if ((i + 1) % 16 == 0) {
                printf("|  %s \n", ascii);
            }
            else if (i + 1 == size) {
                ascii[(i + 1) % 16] = '\0';
                if ((i + 1) % 16 <= 8) {
                    printf(" ");
                }
                for (j = (i + 1) % 16; j < 16; ++j) {
                    printf("   ");
                }
                printf("|  %s \n", ascii);
            }
        }
    }
}

int main(int argc, char **argv)
{
    auto magic = "SGL!";

    std::ifstream in(argv[1], std::ifstream::binary);
    if (in.is_open())
    {
        std::cout << argv[1] << std::endl;

        in.seekg(0, in.end);
        auto file_len = in.tellg();
        in.seekg(0, in.beg);

        auto h1_len = 16;
        char h1[h1_len];
        in.read(h1, h1_len);

        // skip 4 for xor "SGL!"
        for (auto x0 = 4; x0 < h1_len; x0++)
        {
            h1[x0] ^= magic[x0 % 4];
        }

        std::cout << "Header 1 (16):" << std::endl;
        HexDump(h1, h1_len);

        auto h2_offset = *(uint16_t*)(h1 + 12);
        std::cout << "Offset H2: " << h2_offset << std::endl;

        in.seekg(h2_offset, in.beg);

        auto h2_len = 0x67;
        char h2[h2_len];
        in.read(h2, h2_len);

        char* h2_xor = h1 + 14;
        for (auto x0 = 0; x0 < h2_len; x0++)
        {
            h2[x0] ^= h2_xor[x0 % 2];
        }
        std::cout << "Header 2 (" << h2_len << "):" << std::endl;
        HexDump(h2, h2_len);

        auto flen = *(uint32_t*)(h2 + 6);
        std::cout << "File Length: 0x" << std::hex << std::setw(8) << std::setfill('0') << flen << std::endl;

        in.seekg(0x400, in.beg);
        uint32_t binary_start = (int)h1[11];
        in.seekg(binary_start, in.cur);
        std::cout << "Binary Start Offset: 1024 + " << std::dec << binary_start << std::endl;

        uint32_t binary_len = file_len - 0x400L - binary_start;
        char binary[binary_len];
        in.read(binary, binary_len);
        std::cout << "Binary (" << binary_len << "):" << std::endl;
        //HexDump(binary, binary_len);

        std::cout << std::endl;
        /*return;
        auto rh = 1024;
        char h512[rh];
        in.seekg(offset, std::ifstream::beg);
        in.read(h512, rh);

        for(auto x1 = 0; x1 < rh; x1++) 
        {
            h512[x1] = h512[x1] ^ header_xor[x1 % 2];
        }

        auto vp = std::vector<uint8_t>(h512, h512+rh);
        PrintHex(vp.begin(), vp.end());*/
    }
}