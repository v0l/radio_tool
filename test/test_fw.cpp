#include <iostream>
#include <vector>

#include <radio_tool/util.hpp>
#include <radio_tool/fw/fw_factory.hpp>

int main(int argc, char **argv)
{
    if (argc != 3)
    {
        std::cerr << "Usage: " << argv[0] << " <firmware.bin> <radio_model_check>" << std::endl;
        exit(1);
    }
    const char *file = argv[1];
    const char *radio = argv[2];

    std::cout << "Testing: " << file << " (" << radio << ")" << std::endl;

    auto h = radio_tool::fw::FirmwareFactory::GetFirmwareFileHandler(file);
    h->Read(file);

    std::cout << h->ToString();

    if (h->GetRadioModel() != std::string(radio))
    {
        std::cerr << "Firmware model incorrect" << std::endl;
        exit(1);
    }

    //Unwrap and rebuild firmware
    auto write_test_name = "write_test_";
    auto write_test = "write_test_wrapped.bin";
    std::vector<std::pair<uint32_t, uint32_t>> segs;
    for(const auto& seg : h->GetDataSegments())
    {
        std::stringstream ss_name;
        ss_name << write_test_name << "_0x" << std::setw(8) << std::setfill('0') << std::hex << seg.address;

        std::ofstream fw_out;
        fw_out.open(ss_name.str(), std::ios_base::out | std::ios_base::binary);
        if(fw_out.is_open()) 
        {
            fw_out.write((const char*)seg.data.data(), seg.data.size());
            fw_out.close();
            segs.push_back({seg.address, seg.size});
        } 
        else 
        {
            std::cerr << "Failed to open output file: " << ss_name.str() << std::endl;
            exit(1);
        }
    }

    //Create a new firmware from unwrapped segments
    auto fw_new = radio_tool::fw::FirmwareFactory::GetFirmwareModelHandler(h->GetRadioModel());
    fw_new->SetRadioModel(h->GetRadioModel());
    for(const auto& seg : segs)
    {
        std::stringstream ss_name;
        ss_name << write_test_name << "_0x" << std::setw(8) << std::setfill('0') << std::hex << seg.first;

        std::ifstream fw_in;
        fw_in.open(ss_name.str(), std::ios_base::in | std::ios_base::binary);
        if(fw_in.is_open()) 
        {
            fw_in.seekg(0, fw_in.end);
            auto size = fw_in.tellg();
            fw_in.seekg(0, fw_in.beg);

            std::vector<uint8_t> fw_data;
            fw_data.resize(size);
            fw_in.read((char*)fw_data.data(), size);
            fw_in.close();

            fw_new->AppendSegment(seg.first, fw_data);
        } 
        else 
        {
            std::cerr << "Failed to open input file: " << ss_name.str() << std::endl;
            exit(1);
        }
    }
    fw_new->Write(write_test);
    std::ifstream fa(file, std::ios_base::binary);
    std::ifstream fb(write_test, std::ios_base::binary);

    //check length fist
    fa.seekg(0, std::ios_base::end);
    fb.seekg(0, std::ios_base::end);
    auto 
        len_a = fa.tellg(),
        len_b = fb.tellg();
    
    if(len_a != len_b)
    {
        std::cerr << "Write test failed, file lengths dont match: " << len_a << " != " << len_b << std::endl;
        exit(1);
    }
    
    fa.seekg(0, std::ios_base::beg);
    fb.seekg(0, std::ios_base::beg);
    auto pos = 0;
    while(true) 
    {
        if(fa.eof() || fb.eof()) break;

        uint8_t a, b;
        fa.read((char*)&a, 1);
        fb.read((char*)&b, 1);
        if(a != b /* Special checks for TYT firmware */)
        {
            if(pos >= 0x10 && pos <= 0x20 && ((a == 0x00 && b == 0xff) || (a == 0xff && b == 0x00)))
            {
                //if inside the header and some padding value is different
                //just continue, we wont write the header exactly the same
                //for the radio model string
                pos++;
                continue;
            }
            if(pos == 0x30) 
            {
                // when we write the firmware we dont always use the exact same counter magic,
                // this is because some radios (MD2017) use multipe counter magic values
                // for different version of the firmare (CSV/REC), if we wrap a new firmware file we just
                // use the first one which matches in model name (MD2017 / MD2017 GPS).
                // In this case we should just check that the model is the same
                char ca[3];
                char cb[3];
                fa.read(ca, 3);
                fb.read(cb, 3);
        
                auto va = std::vector<uint8_t>(&ca[0], &ca[(int)a]);
                auto vb = std::vector<uint8_t>(&cb[0], &cb[(int)b]);
                va.insert(va.begin(), a);
                vb.insert(vb.begin(), b);
                //radio_tool::PrintHex(va.begin(), va.end());
                //radio_tool::PrintHex(vb.begin(), vb.end());

                auto r_a = radio_tool::fw::TYTFW::GetRadioFromMagic(va);
                auto r_b = radio_tool::fw::TYTFW::GetRadioFromMagic(vb);
                if(r_a != r_b)
                {
                    std::cerr << "Model magic is not the same: " << r_a << " != " << r_b << std::endl;
                    exit(1);
                }
                else 
                {
                    pos += 3;
                    continue;
                }
            }
            if(pos >= 0x7c && pos <= 0x80 && a == 0xff)
            {
                //if original firmware didnt fill the n_regions just skip this
                //on older firmware files n_regions is 0xffffffff instead of 0x01
                pos++;
                continue;
            }
            std::stringstream ssmsg;
            ssmsg << "Write test failed, files are not the same @ 0x"
                << std::hex << ++pos << " [0x" << std::hex << (int)a << " != 0x" << std::hex << (int)b << "]";
            
            std::cerr << ssmsg.str() << std::endl;
            exit(1);
        }
        pos++;
    }

    std::cout << "Passed!" << std::endl;
    exit(0);
}