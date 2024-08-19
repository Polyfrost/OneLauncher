#include "gamemode_client.h"

int gamemode_start_for_wrapper(pid_t pid) {
	return gamemode_request_start_for(pid);
}
