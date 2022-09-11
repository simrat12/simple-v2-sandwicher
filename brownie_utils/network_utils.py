from brownie import network


def change_network(name):
    if network.is_connected():
        network.disconnect()
    while not network.is_connected():
        try:
            network.connect(name)
        except Exception as e:
            print("Unable to connect, trying again.", e)
