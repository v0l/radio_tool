extern "C"
{
#include "export.h"
}

#include <radio_tool/radio/radio_factory.hpp>
#include <radio_tool/fw/fw_factory.hpp>
#include <memory>
#include <vector>
#include <codecvt>
#include <string>

using namespace radio_tool::radio;
using namespace radio_tool::fw;

struct radiotool_ctx
{
    RadioFactory *factory;
};

int radiotool_init(radiotool_ctx_ptr *ctx)
{
    *ctx = (radiotool_ctx_ptr)malloc(sizeof(radiotool_ctx));
    if (*ctx == NULL)
    {
        return RDTS_INVALID_CTX;
    }

    (*ctx)->factory = new RadioFactory();
    return RDTS_OK;
}

int radiotool_close(radiotool_ctx_ptr ctx)
{
    if (ctx == NULL)
    {
        return RDTS_INVALID_CTX;
    }

    delete ctx->factory;
    return RDTS_OK;
}

int radiotool_list_devices(radiotool_ctx_ptr ctx, radiotool_radio_info **infos)
{
    auto err = 0;
    if (ctx == nullptr || ctx->factory == nullptr)
    {
        return RDTS_INVALID_CTX;
    }

    auto devices = ctx->factory->ListDevices();
    auto converter = std::wstring_convert<std::codecvt_utf8<wchar_t>>();
    if (devices.size() > 0)
    {
#ifdef DEBUG_PRINT
        fprintf(stderr, "size=%d, count=%d\n", sizeof(radiotool_radio_info), devices.size());
#endif
        *infos = (radiotool_radio_info *)calloc(devices.size(), sizeof(radiotool_radio_info));

        for (auto &d : devices)
        {
            auto manu = converter.to_bytes(d->manufacturer);
            auto model = converter.to_bytes(d->model);

            auto c_manu = (char *)calloc(manu.size() + 1, sizeof(char));
            auto c_model = (char *)calloc(model.size() + 1, sizeof(char));
            auto c_port = (char *)calloc(d->port.size() + 1, sizeof(char));

            strcpy(c_manu, manu.c_str());
            strcpy(c_model, model.c_str());
            strcpy(c_port, d->port.c_str());

            radiotool_radio_info new_info = {c_manu, c_model, c_port};
            auto infx = (*infos + err);
            memcpy(infx, &new_info, sizeof(radiotool_radio_info));
#ifdef DEBUG_PRINT
            fprintf(stderr, "port=%s, manu=%s, model=%s\n", infx->port, infx->manufacturer, infx->model);
#endif
            err++;
        }
    }
    return err;
}

int radiotool_free_radio_infos(radiotool_radio_info *infos, int n)
{
    if (infos == NULL)
    {
        return RDTS_INVALID_CTX;
    }

    for (int x = 0; x < n; x++)
    {
        free((void *)infos[x].manufacturer);
        free((void *)infos[x].model);
        free((void *)infos[x].port);
    }

    free(infos);

    return RDTS_OK;
}

int radiotool_load_firmware(radiotool_ctx_ptr ctx, const char *file, radiotool_firmware_info **info)
{
    if (ctx == nullptr)
    {
        return RDTS_INVALID_CTX;
    }
    auto handler = FirmwareFactory::GetFirmwareFileHandler(file);
    if (handler == nullptr)
    {
        return RDTS_FIRMWARE_NOT_SUPPORTED;
    }

    handler->Read(file);
    auto model = handler->GetRadioModel();

    *info = (radiotool_firmware_info *)malloc(sizeof(radiotool_firmware_info));

    auto c_model = (char *)malloc(model.size() + 1);
    strcpy(c_model, model.c_str());

    **info = {c_model};

    return RDTS_OK;
}