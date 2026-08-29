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
 *
 * Firmware wrap/unwrap round trips which do not need any external firmware
 * files, plus the malformed input cases the readers have to survive.
 */
#include <radio_tool/fw/fw_factory.hpp>

#include <cstdio>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

using namespace radio_tool::fw;

namespace
{
    auto err = 0;

    auto Fail(const std::string &msg) -> void
    {
        std::cerr << "FAIL: " << msg << std::endl;
        err = 1;
    }

    auto Check(const bool &ok, const std::string &msg) -> void
    {
        if (!ok)
        {
            Fail(msg);
        }
    }

    /**
     * Deterministic pseudo random data, so a failure is reproducible
     */
    auto MakeData(const size_t &len, const uint32_t &seed) -> std::vector<uint8_t>
    {
        std::vector<uint8_t> ret;
        ret.reserve(len);
        auto x = seed | 1u;
        for (size_t ix = 0; ix < len; ix++)
        {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            ret.push_back((uint8_t)(x & 0xff));
        }
        return ret;
    }

    auto WriteFile(const std::string &name, const std::vector<uint8_t> &data) -> std::string
    {
        std::ofstream f(name, std::ios_base::out | std::ios_base::binary);
        f.write((const char *)data.data(), data.size());
        f.close();
        return name;
    }

    auto ReadFile(const std::string &name) -> std::vector<uint8_t>
    {
        std::ifstream f(name, std::ios_base::in | std::ios_base::binary | std::ios_base::ate);
        auto size = (size_t)f.tellg();
        f.seekg(0);
        std::vector<uint8_t> ret(size);
        f.read((char *)ret.data(), size);
        return ret;
    }

    /**
     * Wrap segments for a model, read the file back and check every segment
     * comes out byte for byte the same as it went in
     */
    auto RoundTrip(const std::string &model, const std::vector<std::pair<uint32_t, std::vector<uint8_t>>> &segments) -> void
    {
        auto file = "test_fw_rt_" + model + ".bin";

        try
        {
            auto fw = FirmwareFactory::GetFirmwareModelHandler(model);
            //deliberately in this order: the CLI selects the model before it
            //has read any segment files, which used to leave the SGL header
            //claiming a length of zero
            fw->SetRadioModel(model);
            for (const auto &s : segments)
            {
                fw->AppendSegment(s.first, s.second);
            }
            fw->Encrypt();
            fw->Write(file);
        }
        catch (const std::exception &e)
        {
            Fail(model + " wrap threw: " + e.what());
            return;
        }

        try
        {
            auto fw = FirmwareFactory::GetFirmwareFileHandler(file);
            fw->Read(file);
            fw->Decrypt();

            Check(fw->GetRadioModel() == model, model + ": model did not survive the round trip, got " + fw->GetRadioModel());

            auto read_segments = fw->GetDataSegments();
            Check(read_segments.size() == segments.size(), model + ": wrong segment count");

            for (size_t ix = 0; ix < read_segments.size() && ix < segments.size(); ix++)
            {
                const auto &want = segments[ix];
                const auto &got = read_segments[ix];

                Check(got.address == want.first, model + ": segment address changed");
                Check(got.data.size() >= want.second.size(), model + ": segment is shorter than the input");
                Check(std::equal(want.second.begin(), want.second.end(), got.data.begin()),
                      model + ": segment data changed in the round trip");

                //anything past the input is padding and must be 0xff
                for (size_t p = want.second.size(); p < got.data.size(); p++)
                {
                    if (got.data[p] != 0xff)
                    {
                        Fail(model + ": padding is not 0xff");
                        break;
                    }
                }
            }

            //a file is always compatible with itself
            auto same = FirmwareFactory::GetFirmwareFileHandler(file);
            same->Read(file);
            Check(fw->IsCompatible(same.get()), model + ": a firmware file is not compatible with itself");
        }
        catch (const std::exception &e)
        {
            Fail(model + " unwrap threw: " + e.what());
        }

        std::remove(file.c_str());
    }
}

auto main(int, char **) -> int
{
    // TYT firmware, single and multi segment
    RoundTrip("DM1701", {{0x0800c000, MakeData(0x4000, 1)}});
    RoundTrip("UV3X0", {{0x0800c000, MakeData(0x4000, 2)}, {0x08040000, MakeData(0x2000, 3)}});
    RoundTrip("MD9600", {{0x0800c000, MakeData(0x1234, 4)}});

    // SGL firmware
    RoundTrip("GD77", {{0x0, MakeData(0x8000, 5)}});
    RoundTrip("DM1801", {{0x0, MakeData(0x1000, 6)}});

    // a TYT file and an SGL file are never compatible with each other
    {
        auto tyt_file = std::string("test_fw_mix_tyt.bin");
        auto sgl_file = std::string("test_fw_mix_sgl.bin");
        try
        {
            auto tyt = FirmwareFactory::GetFirmwareModelHandler("DM1701");
            tyt->SetRadioModel("DM1701");
            tyt->AppendSegment(0x0800c000, MakeData(0x2000, 7));
            tyt->Encrypt();
            tyt->Write(tyt_file);

            auto sgl = FirmwareFactory::GetFirmwareModelHandler("GD77");
            sgl->SetRadioModel("GD77");
            sgl->AppendSegment(0x0, MakeData(0x2000, 8));
            sgl->Encrypt();
            sgl->Write(sgl_file);

            auto a = FirmwareFactory::GetFirmwareFileHandler(tyt_file);
            a->Read(tyt_file);
            auto b = FirmwareFactory::GetFirmwareFileHandler(sgl_file);
            b->Read(sgl_file);

            Check(!a->IsCompatible(b.get()), "a TYT file reported itself compatible with an SGL file");
            Check(!b->IsCompatible(a.get()), "an SGL file reported itself compatible with a TYT file");
        }
        catch (const std::exception &e)
        {
            Fail(std::string("mixed compatibility check threw: ") + e.what());
        }
        std::remove(tyt_file.c_str());
        std::remove(sgl_file.c_str());
    }

    // firmware for two different TYT radios is not compatible
    {
        auto a_file = std::string("test_fw_dm1701.bin");
        auto b_file = std::string("test_fw_md9600.bin");
        try
        {
            for (const auto &m : {std::make_pair(std::string("DM1701"), a_file),
                                  std::make_pair(std::string("MD9600"), b_file)})
            {
                auto fw = FirmwareFactory::GetFirmwareModelHandler(m.first);
                fw->SetRadioModel(m.first);
                fw->AppendSegment(0x0800c000, MakeData(0x2000, 9));
                fw->Encrypt();
                fw->Write(m.second);
            }

            auto a = FirmwareFactory::GetFirmwareFileHandler(a_file);
            a->Read(a_file);
            auto b = FirmwareFactory::GetFirmwareFileHandler(b_file);
            b->Read(b_file);

            Check(!a->IsCompatible(b.get()), "firmware for two different radios reported compatible");
        }
        catch (const std::exception &e)
        {
            Fail(std::string("model compatibility check threw: ") + e.what());
        }
        std::remove(a_file.c_str());
        std::remove(b_file.c_str());
    }

    // encrypt then decrypt must be the identity for every handler
    {
        for (const auto &model : {std::string("DM1701"), std::string("GD77"), std::string("HD1")})
        {
            try
            {
                auto plain = MakeData(0x2000, 10);

                auto fw = FirmwareFactory::GetFirmwareModelHandler(model);
                fw->SetRadioModel(model);
                fw->AppendSegment(0x0, plain);
                fw->Encrypt();
                auto cipher = fw->GetData();
                Check(cipher.size() >= plain.size(), model + ": data shrank while encrypting");
                Check(!std::equal(plain.begin(), plain.end(), cipher.begin()), model + ": encrypt did not change the data");

                fw->Decrypt();
                const auto &back = fw->GetData();
                Check(std::equal(plain.begin(), plain.end(), back.begin()), model + ": decrypt(encrypt(x)) != x");
            }
            catch (const std::exception &e)
            {
                Fail(model + " cipher round trip threw: " + e.what());
            }
        }
    }

    // truncated and malformed files must throw rather than read out of bounds
    {
        const std::vector<std::pair<std::string, std::vector<uint8_t>>> bad = {
            {"test_fw_empty.bin", {}},
            {"test_fw_short.bin", MakeData(3, 11)},
            {"test_fw_sgl_hdr_only.bin", {'S', 'G', 'L', '!'}},
            {"test_fw_junk.bin", MakeData(0x400, 12)},
        };

        for (const auto &b : bad)
        {
            auto file = WriteFile(b.first, b.second);
            try
            {
                //any handler may claim the file (the Yaesu one takes anything),
                //but nothing may crash or read past the end of the buffer
                auto fw = FirmwareFactory::GetFirmwareFileHandler(file);
                fw->Read(file);
                (void)fw->ToString();
            }
            catch (const std::exception &)
            {
            }
            std::remove(file.c_str());
        }
    }

    // an SGL file truncated after the header must be rejected, not padded with
    // whatever happens to be in memory
    {
        auto file = std::string("test_fw_sgl_trunc.bin");
        try
        {
            auto fw = FirmwareFactory::GetFirmwareModelHandler("GD77");
            fw->SetRadioModel("GD77");
            fw->AppendSegment(0x0, MakeData(0x2000, 13));
            fw->Encrypt();
            fw->Write(file);

            auto data = ReadFile(file);
            data.resize(data.size() - 0x800);
            WriteFile(file, data);

            auto trunc = FirmwareFactory::GetFirmwareFileHandler(file);
            trunc->Read(file);
            Fail("a truncated SGL file was accepted");
        }
        catch (const std::exception &)
        {
        }
        std::remove(file.c_str());
    }

    if (err == 0)
    {
        std::cerr << "PASS" << std::endl;
    }
    return err;
}
