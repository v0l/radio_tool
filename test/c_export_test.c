#include "export.h"
#include "stdio.h"

int main(int argc, char **argv)
{
    int err = 0;
    radiotool_ctx_ptr ctx = 0;
    if ((err = radiotool_init(&ctx)) != RDTS_OK)
    {
        fprintf(stderr, "Failed to init radiotool: %d\n", err);
        return 1;
    }

    int n = 0;
    radiotool_radio_info *infos = 0;
    if ((n = radiotool_list_devices(ctx, &infos)) < RDTS_OK)
    {
        fprintf(stderr, "Failed to get device list: %d\n", n);
        return 1;
    }

    fprintf(stdout, "N=%i, infos=%p\n", n, infos);
    for (int x = 0; x < n; x++)
    {
        radiotool_radio_info *infx = infos + x;
        fprintf(stdout, "[%d](%s) %s %s\n", x, infx->port, infx->manufacturer, infx->model);
    }

    if ((err = radiotool_free_radio_infos(infos, n) != RDTS_OK))
    {
        fprintf(stderr, "Failed to free radio list: %d\n", err);
        return 1;
    }
    radiotool_close(ctx);
    return 0;
}