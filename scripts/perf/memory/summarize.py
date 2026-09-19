"""Summarize fresh-process results without removing helpers or failed runs."""
import argparse
import json
import math
from pathlib import Path
import statistics

MIB=2**20
def total(sample): return sum(row['footprint_bytes'] for row in sample['processes'])/MIB
def spread(values):
    return {'values':values,'median':statistics.median(values),'min':min(values),'max':max(values)}
def percentile(values,p): return sorted(values)[max(0,math.ceil(len(values)*p)-1)]
def latency(values):
    return {'n':len(values),'p50_ms':statistics.median(values),'p95_ms':percentile(values,.95),
            'min_ms':min(values),'max_ms':max(values)}

def component(process,host):
    if process['pid']==host:return 'host'
    names={'com.apple.WebKit.WebContent':'web_content','com.apple.WebKit.GPU':'web_gpu',
           'com.apple.WebKit.Networking':'web_network'}
    return names.get(Path(process['executable']).name,'other_helpers')

def summarize(directory,partial,final=False,candidate='final'):
    groups={}
    errors=[]
    for path in sorted(directory.glob('*.json')):
        data=json.loads(path.read_text())
        if data.get('smoke_only') or data.get('error'):
            errors.append({'file':path.name,'error':data.get('error','smoke run')})
            continue
        assert data['latency_trace_disabled'], path
        assert not data['response_size_audit'], path
        groups.setdefault((data['build'],data['profile']),[]).append((path.name,data))
    output={'metric':'macOS physical footprint, host and every attributed helper',
            'spread':'range of fresh-process run medians','excluded_runs':errors,'configurations':{}}
    corpus=set()
    for (build,profile),runs in sorted(groups.items()):
        runs.sort(key=lambda pair:pair[1]['trial'])
        assert partial or len(runs)==5,(build,profile,len(runs))
        binaries={data['executable_sha256'] for _,data in runs}
        assert len(binaries)==1,(build,binaries)
        summary={'binary_sha256':next(iter(binaries)),'complete':len(runs)==5,'runs':[],
                 'footprint':{},'components':{},'latency':{}}
        for filename,data in runs:
            corpus.add(data['corpus_sha256'])
            assert data['page_identity']['appearance']=='light'
            assert all(row['success'] for row in data['queries']['samples']+data['reopen']['samples'])
            row={'file':filename,'trial':data['trial'],'peak_mib':max(map(total,data['workload_samples'])),
                 'peak_before_settings_mib':max(total(sample) for sample in data['workload_samples'] if sample['monotonic']<=data['settings_started_monotonic']),
                 'representative_processes':{},'components':{}}
            for state in ['visible','hidden','after_settings']:
                if state not in data: continue
                samples=data[state]['samples']
                assert len(samples)==31,(filename,state)
                row[state+'_mib']=statistics.median(map(total,samples))
                row['components'][state]={name:statistics.median(
                    sum(process['footprint_bytes'] for process in sample['processes']
                        if component(process,data['pid'])==name)/MIB for sample in samples)
                    for name in ['host','web_content','web_gpu','web_network','other_helpers']}
                middle=sorted(samples,key=total)[len(samples)//2]
                row['representative_processes'][state]=[{'pid':process['pid'],
                    'name':Path(process['executable']).name,'mib':process['footprint_bytes']/MIB}
                    for process in middle['processes']]
            summary['runs'].append(row)
        for state in ['visible','hidden','after_settings','peak','peak_before_settings']:
            values=[row[state+'_mib'] for row in summary['runs'] if state+'_mib' in row]
            if values:summary['footprint'][state]=spread(values)
            if state in summary['runs'][0]['components']:
                summary['components'][state]={name:spread([row['components'][state][name] for row in summary['runs']])
                    for name in summary['runs'][0]['components'][state]}
        for group in ['apps','arithmetic','reopen','settings','settings_first','settings_reopen']:
            values=[]
            per_run=[]
            per_run_p95=[]
            for _,data in runs:
                if group.startswith('settings'):
                    current=data.get('settings_open_ms',[])
                    if group=='settings_first':current=current[:1]
                    if group=='settings_reopen':current=current[1:]
                else:
                    rows=data['reopen' if group=='reopen' else 'queries']['samples']
                    current=[row['ms'] for row in rows if not row['warmup'] and (group=='reopen' or row['group']==group)]
                if current:
                    values.extend(current)
                    per_run.append(statistics.median(current))
                    per_run_p95.append(percentile(current,.95))
            if values:summary['latency'][group]={**latency(values),'run_p50':spread(per_run),
                                                'run_p95':spread(per_run_p95)}
        output['configurations'][build+'/'+profile]=summary
    assert len(corpus)<=1, 'Application corpus changed between runs'
    if not partial:
        expected = ({'baseline2/controlled','baseline2/daily',candidate+'/controlled',candidate+'/daily'}
                    if final else {'baseline2/controlled','baseline2/daily','p1/controlled',
                                   'p2/controlled','p3a/controlled','p3b/controlled','p3b/daily'})
        assert set(output['configurations']) == expected
    output['corpus_sha256']=next(iter(corpus),None)
    output['comparisons']=[]
    for profile in ['controlled','daily']:
        pairs=[('baseline2',candidate if final else 'p3b')]
        if profile=='controlled' and not final:pairs += [('baseline2','p1'),('p1','p2'),('p2','p3a'),('p3a','p3b')]
        for parent,candidate in pairs:
            before=output['configurations'].get(parent+'/'+profile)
            after=output['configurations'].get(candidate+'/'+profile)
            if not before or not after:continue
            complete=before['complete'] and after['complete']
            comparison={'profile':profile,'parent':parent,'candidate':candidate,'complete':complete,'footprint':{},'latency':{}}
            for state,baseline in before['footprint'].items():
                if state not in after['footprint']:continue
                changed=after['footprint'][state]
                delta=changed['median']-baseline['median']
                noise=max(baseline['max']-baseline['min'],changed['max']-changed['min'])
                comparison['footprint'][state]={'delta_mib':delta,'delta_percent':delta/baseline['median']*100,
                    'same_build_range_mib':noise,'resolves_range':complete and abs(delta)>noise}
            for group in ['apps','arithmetic','reopen']:
                old=before['latency'][group]
                new=after['latency'][group]
                p50_budget=max(2,old['p50_ms']*.05)
                p95_budget=max(5,old['p95_ms']*.10)
                p50_noise=max(old['run_p50']['max']-old['run_p50']['min'],new['run_p50']['max']-new['run_p50']['min'])
                p95_noise=max(old['run_p95']['max']-old['run_p95']['min'],new['run_p95']['max']-new['run_p95']['min'])
                within_budget=new['p50_ms']<=old['p50_ms']+p50_budget and new['p95_ms']<=old['p95_ms']+p95_budget
                resolves_budget=complete and p50_noise<=p50_budget and p95_noise<=p95_budget
                comparison['latency'][group]={'p50_delta_ms':new['p50_ms']-old['p50_ms'],
                    'p95_delta_ms':new['p95_ms']-old['p95_ms'],
                    'p50_budget_ms':p50_budget,'p95_budget_ms':p95_budget,
                    'within_observed_budget':within_budget,
                    'p50_same_build_range_ms':p50_noise,'p95_same_build_range_ms':p95_noise,
                    'verdict':('within budget' if within_budget else 'exceeds budget') if resolves_budget else 'inconclusive'}
            output['comparisons'].append(comparison)
    return output

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('directory',type=Path)
    parser.add_argument('output',type=Path)
    parser.add_argument('--partial',action='store_true')
    parser.add_argument('--final-comparison',action='store_true')
    parser.add_argument('--candidate',default='final')
    args=parser.parse_args()
    args.output.write_text(json.dumps(summarize(args.directory,args.partial,args.final_comparison,args.candidate),indent=2)+'\n')
