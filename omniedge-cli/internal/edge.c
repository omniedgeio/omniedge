#include "n2n/n2n.h"

int edge_configure(n2n_edge_conf_t *conf,
                   char *supernode,
                   char *private_key,
                   int allow_p2p,
                   int allow_routing,
                   char *community_name,
                   int disable_pmtu_discovery,
                   int drop_multicast,
                   char *encrypt_key,
                   int local_port,
                   int mgmt_port,
                   int sn_num,
                   int transop_id,
                   int tos,
                   int register_interval,
                   int register_ttl) {
    edge_init_conf_defaults(conf);
    conf->allow_p2p = allow_p2p;
    conf->allow_routing = allow_routing;
    snprintf((char *) conf->community_name, sizeof(conf->community_name), "%s", community_name);
    conf->disable_pmtu_discovery = disable_pmtu_discovery;
    conf->drop_multicast = drop_multicast;
    conf->encrypt_key = encrypt_key;
    conf->local_port = local_port;
    conf->mgmt_port = mgmt_port;
    conf->transop_id = transop_id;
    conf->tos = tos;
    conf->sn_num = sn_num;
    conf->register_interval = register_interval;
    conf->register_ttl = register_ttl;
    edge_conf_add_supernode(conf,supernode);
    return edge_verify_conf(conf);
}

int edge_start(tuntap_dev *tuntap, n2n_edge_conf_t *conf, int *keep_running) {
    n2n_edge_t *edge;
    int rc;
    edge = edge_init(tuntap, conf, &rc);
    if (edge == NULL) {
        return -1;
    }
    rc = run_edge_loop(edge, keep_running);

    edge_term(edge);
    tuntap_close(tuntap);
    tuntap->device_mask = 0;
    return rc;
}