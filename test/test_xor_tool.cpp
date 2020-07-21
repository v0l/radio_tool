#include <iostream>

#include <radio_tool/fw/fw_factory.hpp>

int main(int argc, char** argv) {
    if (argc != 3) {
        std::cerr << "Usage: " << argv[0] << " <firmware.bin>" << std::endl;
        exit(1);
    }
    const char* file = argv[1];
    
    exit(0);
}