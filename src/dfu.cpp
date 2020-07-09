#include <dfu.hpp>
#include <dfu_exception.hpp>

#include <sstream>
#include <iostream>
#include <iomanip>
#include <locale>
#include <codecvt>
#include <stdio.h>
#include <string.h>
#include <exception>

using namespace tytfw::dfu;

bool DFU::Init()
{
    auto err = libusb_init(&this->usb_ctx);
    if (err != LIBUSB_SUCCESS)
    {
        std::wcerr << libusb_error_name(err) << std::endl;
        return false;
    }

    libusb_set_log_cb(
        this->usb_ctx, [](libusb_context *ctx, enum libusb_log_level level, const char *str) {
            std::wcout << str << std::endl;
        },
        LIBUSB_LOG_CB_CONTEXT);

    return true;
}

auto DFU::ListDevices() -> std::vector<std::wstring>
{
    std::vector<std::wstring> ret;

    libusb_device **devs;
    auto ndev = libusb_get_device_list(this->usb_ctx, &devs);
    int err = LIBUSB_SUCCESS;
    auto n_idx = 0;
    std::wstringstream os;
    if (ndev >= 0)
    {
        for (auto x = 0; x < ndev; x++)
        {
            os.str(L"");
            libusb_device_descriptor desc;
            if (LIBUSB_SUCCESS == (err = libusb_get_device_descriptor(devs[x], &desc)))
            {
                if (desc.idVendor == this->vid && desc.idProduct == this->pid)
                {
                    libusb_device_handle *h;
                    if (LIBUSB_SUCCESS == (err = libusb_open(devs[x], &h)))
                    {
                        os << L"["
                           << std::setfill(L'0') << std::setw(4) << std::hex << desc.idVendor
                           << L":"
                           << std::setfill(L'0') << std::setw(4) << std::hex << desc.idProduct
                           << L"]: idx=" << std::setfill(L'0') << std::setw(3) << std::to_wstring(n_idx) << L", "
                           << GetDeviceString(desc, h);
                        ret.push_back(os.str());
                        libusb_close(h);
                    }
                }
            }
            n_idx++;
        }

        libusb_free_device_list(devs, 1);
    }
    else
    {
        std::wcerr << libusb_error_name(ndev) << std::endl;
    }

    return ret;
}

auto DFU::GetDeviceString(const libusb_device_descriptor &desc, libusb_device_handle *h) const -> std::wstring
{
    std::wstringstream os;

    auto err = 0;
    unsigned char data[42], prd[255], mfg[255];
    memset(prd, 0, 255);
    memset(mfg, 0, 255);

    //??
    libusb_get_string_descriptor(h, 0, 0, data, 42);
    auto lang = data[2] << 8 | data[3];
    size_t prd_len = 0, mfg_len = 0;

    if (0 > (prd_len = libusb_get_string_descriptor(h, desc.iProduct, lang, prd, 255)))
    {
        std::wcerr << libusb_error_name(err) << std::endl;
    }
    if (0 > (mfg_len = libusb_get_string_descriptor(h, desc.iManufacturer, lang, mfg, 255)))
    {
        std::wcerr << libusb_error_name(err) << std::endl;
    }

    os << std::wstring_convert<std::codecvt_utf8<wchar_t>>().from_bytes((const char *)mfg + 1, (const char *)mfg + mfg_len) << L" "
       << std::wstring_convert<std::codecvt_utf8<wchar_t>>().from_bytes((const char *)prd + 1, (const char *)prd + prd_len);

    return os.str();
}

auto DFU::Open(uint16_t idx) -> bool
{
    libusb_device **devs;
    auto ndev = libusb_get_device_list(this->usb_ctx, &devs);
    int err = LIBUSB_SUCCESS;
    if (ndev >= 0 && ndev > idx)
    {
        auto dev = devs[idx];
        libusb_device_descriptor desc;
        if (LIBUSB_SUCCESS == (err = libusb_get_device_descriptor(dev, &desc)))
        {
            if (desc.idVendor == this->vid && desc.idProduct == this->pid)
            {
                libusb_device_handle *h;
                if (LIBUSB_SUCCESS == (err = libusb_open(dev, &h)))
                {
                    this->device = h;

                    std::wcout << L"Opened: " << GetDeviceString(desc, this->device) << std::endl;
                }
            }
            else
            {
                std::wcout << L"Can't open this kind of device" << std::endl;
            }
        }

        if (LIBUSB_SUCCESS != err)
        {
            throw DFUException(libusb_error_name(err));
        }
    }
    else if (ndev < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(ndev));
    }
    else
    {
        std::wcerr << L"Device index must be < " << ndev << std::endl;
    }
    libusb_free_device_list(devs, 1);

    return this->device != nullptr;
}

auto DFU::CustomCT(std::vector<uint8_t> data) const
{
    CheckDevice();
    auto err = libusb_control_transfer(this->device, 0x21, static_cast<uint8_t>(DFURequest::Download), 0, 0, data.data(), data.size(), this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
}

auto DFU::SetAddress(const uint32_t addr) const
{
    std::vector<uint8_t> data = {
        static_cast<uint8_t>(0x21),
        static_cast<uint8_t>(addr & 0xFF),
        static_cast<uint8_t>((addr >> 8) & 0xFF),
        static_cast<uint8_t>((addr >> 16) & 0xFF),
        static_cast<uint8_t>((addr >> 24) & 0xFF)};

    Download(data);
}

auto DFU::Erase(const uint32_t addr) const
{
    std::vector<uint8_t> data = {
        static_cast<uint8_t>(0x41),
        static_cast<uint8_t>(addr & 0xFF),
        static_cast<uint8_t>((addr >> 8) & 0xFF),
        static_cast<uint8_t>((addr >> 16) & 0xFF),
        static_cast<uint8_t>((addr >> 24) & 0xFF)};

    Download(data);
}

auto DFU::Download(std::vector<uint8_t> data) const -> void
{
    CheckDevice();
    auto err = libusb_control_transfer(this->device, 0x21, static_cast<uint8_t>(DFURequest::Download), 0, 0, data.data(), data.size(), this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
}

auto DFU::Upload(const uint16_t size) const -> std::vector<uint8_t>
{
    CheckDevice();
    auto data = std::vector<uint8_t>(size);
    auto err = libusb_control_transfer(this->device, 0xa1, static_cast<uint8_t>(DFURequest::Upload), 0, 0, data.data(), data.size(), this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
    else
    {
        data.resize(err);
    }
    return data;
}

auto DFU::CheckDevice() const -> void
{
    if (this->device == nullptr)
        throw std::runtime_error("Device is not opened");
}