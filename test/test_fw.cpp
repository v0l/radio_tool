#include <iostream>
#include <vector>

#include <radio_tool/util.hpp>
#include <radio_tool/fw/fw_factory.hpp>

auto testFirmwareInfo(std::string original_file, std::string new_file) -> void;

int main(int argc, char** argv)
{
	if (argc != 3)
	{
		std::cerr << "Usage: " << argv[0] << " <firmware.bin> <radio_model_check>" << std::endl;
		exit(1);
	}
	const char* file = argv[1];
	const char* radio = argv[2];

	std::cout << "Testing: " << file << " (" << radio << ")" << std::endl;

	try {
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
		for (const auto& seg : h->GetDataSegments())
		{
			std::stringstream ss_name;
			ss_name << write_test_name << "_0x" << std::setw(8) << std::setfill('0') << std::hex << seg.address;

			std::ofstream fw_out;
			fw_out.open(ss_name.str(), std::ios_base::out | std::ios_base::binary);
			if (fw_out.is_open())
			{
				fw_out.write((const char*)seg.data.data(), seg.data.size());
				fw_out.close();
				segs.push_back({ seg.address, seg.size });
			}
			else
			{
				std::cerr << "Failed to open output file: " << ss_name.str() << std::endl;
				exit(1);
			}
		}

		//Create a new firmware from unwrapped segments
		auto fw_new = radio_tool::fw::FirmwareFactory::GetFirmwareModelHandler(h->GetRadioModel());
		for (const auto& seg : segs)
		{
			std::stringstream ss_name;
			ss_name << write_test_name << "_0x" << std::setw(8) << std::setfill('0') << std::hex << seg.first;

			std::ifstream fw_in;
			fw_in.open(ss_name.str(), fw_in.binary);
			if (fw_in.is_open())
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
		fw_new->SetRadioModel(h->GetRadioModel());
		fw_new->Write(write_test);

		testFirmwareInfo(file, write_test);
	}
	catch (std::exception e) {
		std::cerr << "Error: " << e.what() << std::endl;
		exit(1);
	}
	std::cout << "Passed!" << std::endl;
	exit(0);
}

auto testFirmwareInfo(std::string original_file, std::string new_file) -> void {
	auto fw_old = radio_tool::fw::FirmwareFactory::GetFirmwareFileHandler(original_file);
	auto fw_new = radio_tool::fw::FirmwareFactory::GetFirmwareFileHandler(new_file);

	if (fw_old == nullptr || fw_new == nullptr) {
		std::cerr << "Could not find firmware handler!" << std::endl;
		exit(1);
	}

	fw_old->Read(original_file);
	fw_new->Read(new_file);

	if (fw_old->IsCompatible(fw_new.get())) {
		std::cerr << "Firware headers do not match" << std::endl;
		std::cerr << "== Old ==" << fw_old->ToString() << std::endl;
		std::cerr << "== New ==" << fw_new->ToString() << std::endl;
		exit(1);
	}
}