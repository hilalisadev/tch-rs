/* Training various models on the CIFAR-10 dataset.

   The dataset can be downloaded from https://www.cs.toronto.edu/~kriz/cifar.html, files
   should be placed in the data/ directory.

   The resnet model reaches XX.X% accuracy.
*/

extern crate tch;
use tch::nn::{BatchNorm2D, Conv2D, Func, Linear, ModuleT, SequentialT};
use tch::{nn, Device, Tensor};

fn conv_bn(vs: &nn::Path, c_in: i64, c_out: i64) -> SequentialT {
    let conv2d_cfg = nn::Conv2DConfig {
        padding: 1,
        bias: false,
        ..Default::default()
    };
    SequentialT::new()
        .add(Conv2D::new(vs, c_in, c_out, 3, conv2d_cfg))
        .add(BatchNorm2D::new(vs, c_out, Default::default()))
        .add(Func::new(|x| x.relu()))
}

struct Layer {
    pre: SequentialT,
    block1: SequentialT,
    block2: SequentialT,
}

impl Layer {
    fn new(vs: &nn::Path, c_in: i64, c_out: i64) -> Layer {
        Layer {
            pre: conv_bn(&vs.sub("pre"), c_in, c_out),
            block1: conv_bn(&vs.sub("b1"), c_out, c_out),
            block2: conv_bn(&vs.sub("b2"), c_out, c_out),
        }
    }
}

impl ModuleT for Layer {
    fn forward_t(&self, xs: &Tensor, train: bool) -> Tensor {
        let pre = xs.apply_t(&self.pre, train).max_pool2d_default(2);
        let ys = pre
            .apply_t(&self.block1, train)
            .apply_t(&self.block2, train);
        pre + ys
    }
}

fn fast_resnet(vs: &nn::Path) -> SequentialT {
    SequentialT::new()
        .add(conv_bn(&vs.sub("pre"), 3, 64))
        .add(Layer::new(&vs.sub("layer1"), 64, 128))
        .add(conv_bn(&vs.sub("inter"), 128, 256))
        .add(Func::new(|x| x.max_pool2d_default(2)))
        .add(Layer::new(&vs.sub("layer2"), 256, 512))
        .add(Func::new(|x| x.max_pool2d_default(4)))
        .add(Func::new(|x| x.view(&[x.size()[0] as i64, 512])))
        .add(Linear::new(&vs.sub("linear"), 512, 10))
        .add(Func::new(|x| x * 0.125))
}

pub fn main() {
    let m = tch::vision::cifar::load_dir(std::path::Path::new("data")).unwrap();
    let vs = nn::VarStore::new(Device::cuda_if_available());
    let net = fast_resnet(&vs.root());
    let opt = nn::Optimizer::adam(&vs, 1e-4, Default::default());
    for epoch in 1..6000 {
        let (bimages, blabels) =
            Tensor::random_batch2(&m.train_images, &m.train_labels, 64, vs.device());
        let loss = net
            .forward_t(&bimages, true)
            .cross_entropy_for_logits(&blabels);
        opt.backward_step(&loss);
        if epoch % 50 == 0 {
            let test_accuracy =
                net.batch_accuracy_for_logits(&m.test_images, &m.test_labels, vs.device(), 512);
            println!(
                "epoch: {:4} train loss: {:8.5} test acc: {:5.2}%",
                epoch,
                f64::from(&loss),
                100. * test_accuracy,
            );
        }
    }
}
